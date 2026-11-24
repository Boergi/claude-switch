#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod profiles;

use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use fs2::FileExt;
use profiles::{LoginSession, ProfileId};
use serde::{Deserialize, Serialize};
#[cfg(not(debug_assertions))]
use tauri::{Manager, path::BaseDirectory};

#[derive(Debug, Deserialize)]
struct CliStatus {
    profiles: Vec<CliProfile>,
    desktop_install: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CliProfile {
    name: String,
    desktop: DesktopStatus,
}

#[derive(Debug, Deserialize)]
struct DesktopStatus {
    running: bool,
}

#[derive(Serialize)]
struct AppStatus {
    installed: bool,
    profiles: Vec<ProfileStatus>,
    login: Option<ProfileId>,
}

#[derive(Serialize)]
struct ProfileStatus {
    id: ProfileId,
    running: bool,
    initialized: bool,
}

struct Backend {
    cli: PathBuf,
    home: PathBuf,
    _lock: std::fs::File,
}

impl Backend {
    fn new(_app: &tauri::AppHandle) -> Result<Self> {
        let home = profiles::home()?;
        fs::create_dir_all(home.join("state"))?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(home.join("state/desktop-ui.lock"))?;
        lock.try_lock_exclusive().context(
            "Claude Switch führt gerade eine andere Aktion aus. Versuche es gleich erneut.",
        )?;
        #[cfg(debug_assertions)]
        let cli = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/cswitch");
        #[cfg(not(debug_assertions))]
        let cli = _app.path().resolve("cswitch", BaseDirectory::Resource)?;
        if !cli.is_file() {
            bail!("Das mitgelieferte CLI fehlt. Installiere Claude Switch erneut.");
        }
        Ok(Self {
            cli,
            home,
            _lock: lock,
        })
    }

    fn command(&self, args: &[&str]) -> Result<Vec<u8>> {
        let output = Command::new(&self.cli)
            .args(args)
            .env("CSWITCH_HOME", &self.home)
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .output()
            .context("Das Claude-Switch-CLI konnte nicht gestartet werden.")?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr);
            bail!("CLI-Aktion fehlgeschlagen: {}", message.trim());
        }
        Ok(output.stdout)
    }

    fn cli_status(&self) -> Result<CliStatus> {
        serde_json::from_slice(&self.command(&["status", "--json"])?)
            .context("Die CLI-Version liefert ein nicht unterstütztes Statusformat.")
    }

    fn status(&self) -> Result<AppStatus> {
        let cli = self.cli_status()?;
        let state = profiles::load_state(&self.home)?;
        Ok(AppStatus {
            installed: cli.desktop_install.is_some(),
            profiles: [ProfileId::Work, ProfileId::Personal]
                .into_iter()
                .map(|id| ProfileStatus {
                    id,
                    running: cli
                        .profiles
                        .iter()
                        .any(|p| p.name == id.name() && p.desktop.running),
                    initialized: state.initialized.contains(id.name())
                        && cli.profiles.iter().any(|p| p.name == id.name()),
                })
                .collect(),
            login: state.login.map(|session| session.profile),
        })
    }

    fn open(&self, profile: ProfileId) -> Result<AppStatus> {
        if profiles::load_state(&self.home)?.login.is_some() {
            bail!("Schließe zuerst die laufende Anmeldung ab.");
        }
        profiles::ensure_profiles(&self.home)?;
        self.command(&["desktop", profile.name()])?;
        self.status()
    }

    fn begin_login(&self, profile: ProfileId) -> Result<AppStatus> {
        let mut state = profiles::load_state(&self.home)?;
        if state.login.is_some() {
            bail!("Es läuft bereits eine Anmeldung. Schließe sie zuerst ab.");
        }
        profiles::ensure_profiles(&self.home)?;
        let cli = self.cli_status()?;
        if cli.desktop_install.is_none() {
            bail!("Installiere zuerst Claude Desktop im Programme-Ordner.");
        }
        state.login = Some(LoginSession {
            profile,
            reopen: cli
                .profiles
                .iter()
                .filter(|p| p.desktop.running && p.name != profile.name())
                .map(|p| p.name.clone())
                .collect(),
        });
        profiles::save_state(&self.home, &state)?;
        // Persist before the upstream login closes other instances, so an app restart can recover them.
        self.command(&["login", profile.name(), "--desktop", "--no-wait"])?;
        self.status()
    }

    fn finish_login(&self, completed: bool) -> Result<AppStatus> {
        let mut state = profiles::load_state(&self.home)?;
        if let Some(session) = &state.login {
            let cli = self.cli_status()?;
            for name in &session.reopen {
                if cli.profiles.iter().any(|p| p.name == *name) {
                    self.command(&["desktop", name])?;
                }
            }
            if completed {
                state.initialized.insert(session.profile.name().into());
            }
            state.login = None;
            profiles::save_state(&self.home, &state)?;
        }
        self.status()
    }
}

async fn run_command<T: Send + 'static>(
    app: tauri::AppHandle,
    operation: impl FnOnce(Backend) -> Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || operation(Backend::new(&app)?))
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| format!("{err:#}"))
}

#[tauri::command]
async fn app_status(app: tauri::AppHandle) -> Result<AppStatus, String> {
    run_command(app, |backend| backend.status()).await
}

#[tauri::command]
async fn open_profile(app: tauri::AppHandle, profile: ProfileId) -> Result<AppStatus, String> {
    run_command(app, move |backend| backend.open(profile)).await
}

#[tauri::command]
async fn begin_login(app: tauri::AppHandle, profile: ProfileId) -> Result<AppStatus, String> {
    run_command(app, move |backend| backend.begin_login(profile)).await
}

#[tauri::command]
async fn finish_login(app: tauri::AppHandle, completed: bool) -> Result<AppStatus, String> {
    run_command(app, move |backend| backend.finish_login(completed)).await
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            app_status,
            open_profile,
            begin_login,
            finish_login
        ])
        .run(tauri::generate_context!())
        .expect("Claude Switch konnte nicht gestartet werden");
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> (tempfile::TempDir, Backend) {
        let temp = tempfile::tempdir().unwrap();
        let cli = temp.path().join("cswitch");
        fs::write(&cli, r##"#!/bin/sh
printf '%s\n' "$*" >> "$CSWITCH_HOME/calls"
if [ "$1" = status ]; then
  printf '%s\n' '{"profiles":[{"name":"work","desktop":{"running":false}},{"name":"personal","desktop":{"running":true}}],"desktop_install":{}}'
fi
if [ "$1" = login ] && [ -f "$CSWITCH_HOME/fail-login" ]; then
  echo 'login failed' >&2
  exit 1
fi
if [ "$1" = desktop ] && [ -f "$CSWITCH_HOME/fail-reopen" ]; then
  echo 'reopen failed' >&2
  exit 1
fi
"##).unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(temp.path().join("lock"))
            .unwrap();
        let backend = Backend {
            cli,
            home: temp.path().to_path_buf(),
            _lock: lock,
        };
        (temp, backend)
    }

    #[test]
    fn login_completion_reopens_previous_profiles_and_records_confirmation() {
        let (temp, backend) = fixture();
        backend.begin_login(ProfileId::Work).unwrap();
        let pending = profiles::load_state(temp.path()).unwrap();
        assert_eq!(pending.login.unwrap().reopen, vec!["personal"]);
        assert!(backend.open(ProfileId::Personal).is_err());
        backend.finish_login(true).unwrap();
        let saved = profiles::load_state(temp.path()).unwrap();
        assert!(saved.login.is_none());
        assert!(saved.initialized.contains("work"));
        let calls = fs::read_to_string(temp.path().join("calls")).unwrap();
        assert!(calls.contains("login work --desktop --no-wait\n"));
        assert!(calls.contains("desktop personal\n"));
    }

    #[test]
    fn cancellation_restores_profiles_without_marking_login_complete() {
        let (temp, backend) = fixture();
        backend.begin_login(ProfileId::Work).unwrap();
        backend.finish_login(false).unwrap();
        let saved = profiles::load_state(temp.path()).unwrap();
        assert!(saved.login.is_none());
        assert!(saved.initialized.is_empty());
        assert!(
            fs::read_to_string(temp.path().join("calls"))
                .unwrap()
                .contains("desktop personal\n")
        );
    }

    #[test]
    fn failed_login_keeps_recovery_information() {
        let (temp, backend) = fixture();
        fs::write(temp.path().join("fail-login"), "").unwrap();
        assert!(backend.begin_login(ProfileId::Work).is_err());
        assert!(profiles::load_state(temp.path()).unwrap().login.is_some());
        backend.finish_login(false).unwrap();
        assert!(profiles::load_state(temp.path()).unwrap().login.is_none());
    }

    #[test]
    fn failed_reopen_can_be_retried_without_losing_recovery_information() {
        let (temp, backend) = fixture();
        backend.begin_login(ProfileId::Work).unwrap();
        fs::write(temp.path().join("fail-reopen"), "").unwrap();
        assert!(backend.finish_login(true).is_err());
        assert!(profiles::load_state(temp.path()).unwrap().login.is_some());
        fs::remove_file(temp.path().join("fail-reopen")).unwrap();
        backend.finish_login(true).unwrap();
        assert!(
            profiles::load_state(temp.path())
                .unwrap()
                .initialized
                .contains("work")
        );
    }
}
