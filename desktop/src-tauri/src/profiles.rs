use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, Table, value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileId {
    Work,
    Personal,
}

impl ProfileId {
    pub fn name(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Personal => "personal",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Work => "Arbeit",
            Self::Personal => "Privat",
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct UiState {
    pub initialized: BTreeSet<String>,
    pub login: Option<LoginSession>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginSession {
    pub profile: ProfileId,
    pub reopen: Vec<String>,
}

pub fn home() -> Result<PathBuf> {
    match std::env::var_os("CSWITCH_HOME").filter(|v| !v.is_empty()) {
        Some(path) => Ok(PathBuf::from(path)),
        None => Ok(dirs::home_dir()
            .context("Dein Benutzerordner wurde nicht gefunden.")?
            .join(".cswitch")),
    }
}

pub fn load_state(home: &Path) -> Result<UiState> {
    let path = home.join("state/desktop-ui.json");
    match fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).context("Die App-Einstellungen sind beschädigt.")
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(UiState::default()),
        Err(err) => Err(err).context("Die App-Einstellungen konnten nicht gelesen werden."),
    }
}

pub fn save_state(home: &Path, state: &UiState) -> Result<()> {
    atomic_write(
        &home.join("state/desktop-ui.json"),
        &serde_json::to_vec_pretty(state)?,
    )
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().context("Ungültiger Speicherort.")?)?;
    let temporary = path.with_extension("desktop-tmp");
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(temporary, path).context("Die Einstellungen konnten nicht gespeichert werden.")
}

pub fn ensure_profiles(home: &Path) -> Result<()> {
    let path = home.join("config.toml");
    let original = match fs::read_to_string(&path) {
        Ok(raw) => Some(raw),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(err.into()),
    };
    let mut document = original
        .as_deref()
        .unwrap_or("")
        .trim_start_matches('\u{feff}')
        .parse::<DocumentMut>()
        .context("config.toml ist ungültig. Die Datei wurde nicht verändert.")?;
    if document
        .get("version")
        .and_then(Item::as_integer)
        .is_some_and(|version| version != 1)
    {
        bail!("Diese CLI-Konfiguration hat eine noch nicht unterstützte Version.");
    }
    if let Some(profiles) = document.get("profiles") {
        if !profiles.is_table() {
            bail!("Der profiles-Abschnitt in config.toml ist ungültig.");
        }
    } else {
        document["profiles"] = Item::Table(Table::new());
    }
    let mut changed = false;
    for id in [ProfileId::Work, ProfileId::Personal] {
        if document["profiles"].get(id.name()).is_some() {
            continue;
        }
        let mut profile = Table::new();
        profile["label"] = value(id.label());
        profile["color"] = value(if id == ProfileId::Work {
            "#537767"
        } else {
            "#B86549"
        });
        profile["code_storage"] = value("isolated");
        profile["desktop_storage"] = value("isolated");
        profile["desktop_shares_code"] = value(true);
        document["profiles"][id.name()] = Item::Table(profile);
        changed = true;
    }
    validate_isolation(&document, home)?;
    if !changed {
        return Ok(());
    }
    save_config(home, original.as_deref(), &document, "before-desktop")
}

fn save_config(
    home: &Path,
    original: Option<&str>,
    document: &DocumentMut,
    backup_name: &str,
) -> Result<()> {
    let path = home.join("config.toml");
    fs::create_dir_all(home)?;
    if let Some(raw) = &original {
        // The upstream CLI does not take our lock; refuse a detected concurrent edit.
        if fs::read_to_string(&path)? != *raw {
            bail!("Die CLI-Einstellungen wurden gerade geändert. Versuche es erneut.");
        }
        let backup = home.join(format!("config.toml.{backup_name}"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&backup) {
            Ok(mut file) => {
                use std::io::Write;
                file.write_all(raw.as_bytes())?;
                file.sync_all()?;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err.into()),
        }
    }
    atomic_write(&path, document.to_string().as_bytes())
}

pub fn system_desktop_dir() -> Result<PathBuf> {
    Ok(dirs::data_dir()
        .context("Der Claude-Datenordner wurde nicht gefunden.")?
        .join("Claude"))
}

pub fn uses_existing_personal(home: &Path) -> Result<bool> {
    let path = home.join("config.toml");
    if !path.exists() {
        return Ok(false);
    }
    let document = fs::read_to_string(path)?
        .trim_start_matches('\u{feff}')
        .parse::<DocumentMut>()?;
    Ok(document
        .get("profiles")
        .and_then(|p| p.get("personal"))
        .and_then(|p| p.get("desktop_storage"))
        .and_then(Item::as_str)
        == Some("system"))
}

pub fn adopt_personal(home: &Path, existing: &Path) -> Result<()> {
    if !existing.join("config.json").is_file() {
        bail!("Es wurde kein bisheriges Claude-Desktop-Profil gefunden.");
    }
    ensure_profiles(home)?;
    let original = fs::read_to_string(home.join("config.toml"))?;
    let mut document = original
        .trim_start_matches('\u{feff}')
        .parse::<DocumentMut>()?;
    for (name, profile) in document["profiles"]
        .as_table()
        .context("Ungültige Profile.")?
        .iter()
    {
        if name != "personal"
            && profile.get("desktop_storage").and_then(Item::as_str) == Some("system")
        {
            bail!(
                "Profil '{name}' verwendet bereits das bisherige Claude. Ändere zuerst diese Zuordnung im CLI."
            );
        }
    }
    document["profiles"]["personal"]["desktop_storage"] = value("system");
    document["profiles"]["personal"]["code_storage"] = value("system");
    validate_isolation(&document, home)?;
    save_config(home, Some(&original), &document, "before-existing-personal")
}

fn validate_isolation(document: &DocumentMut, home: &Path) -> Result<()> {
    let mut used = BTreeSet::new();
    for id in [ProfileId::Work, ProfileId::Personal] {
        let profile = &document["profiles"][id.name()];
        if !profile.is_table() {
            bail!("Profil '{}' ist ungültig.", id.name());
        }
        let storage = profile
            .get("desktop_storage")
            .and_then(Item::as_str)
            .unwrap_or("isolated");
        let path = match storage {
            "isolated" => home.join("profiles").join(id.name()).join("desktop"),
            "system" if id == ProfileId::Personal => system_desktop_dir()?,
            "system" => bail!(
                "Arbeit muss einen getrennten Desktop-Ordner verwenden. Stelle desktop_storage für work im CLI auf isolated."
            ),
            custom => {
                let path = PathBuf::from(custom);
                if !path.is_absolute() {
                    bail!(
                        "Der Desktop-Speicherort von '{}' muss absolut oder isolated sein.",
                        id.name()
                    );
                }
                path
            }
        };
        let canonical = fs::canonicalize(&path).unwrap_or(path);
        let normalized = canonical
            .to_string_lossy()
            .trim_end_matches('/')
            .to_lowercase();
        if !used.insert(normalized) {
            bail!(
                "Arbeit und Privat verwenden denselben Desktop-Ordner. Wähle im CLI getrennte Speicherorte."
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_two_isolated_profiles_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        ensure_profiles(temp.path()).unwrap();
        let path = temp.path().join("config.toml");
        let first = fs::read_to_string(&path).unwrap();
        let doc = first.parse::<DocumentMut>().unwrap();
        assert_eq!(
            doc["profiles"]["work"]["desktop_storage"].as_str(),
            Some("isolated")
        );
        assert_eq!(
            doc["profiles"]["personal"]["desktop_storage"].as_str(),
            Some("isolated")
        );
        ensure_profiles(temp.path()).unwrap();
        assert_eq!(first, fs::read_to_string(path).unwrap());
    }

    #[test]
    fn preserves_existing_settings_comments_and_unknown_keys() {
        let temp = tempfile::tempdir().unwrap();
        let original = "# Meine Konfiguration\nversion = 1\nfuture_setting = 42\n[profiles.work]\nlabel = 'Firma'\ndesktop_storage = 'isolated'\n[profiles.client]\nlabel = 'Kunde'\n";
        fs::write(temp.path().join("config.toml"), original).unwrap();
        ensure_profiles(temp.path()).unwrap();
        let saved = fs::read_to_string(temp.path().join("config.toml")).unwrap();
        assert!(saved.starts_with("# Meine Konfiguration"));
        let doc = saved.parse::<DocumentMut>().unwrap();
        assert_eq!(doc["future_setting"].as_integer(), Some(42));
        assert_eq!(doc["profiles"]["work"]["label"].as_str(), Some("Firma"));
        assert_eq!(doc["profiles"]["client"]["label"].as_str(), Some("Kunde"));
        assert_eq!(
            fs::read_to_string(temp.path().join("config.toml.before-desktop")).unwrap(),
            original
        );
    }

    #[test]
    fn rejects_invalid_config_without_writing() {
        for original in [
            "[invalid",
            "version = 2",
            "profiles = 'bad'",
            "[profiles.work]\ndesktop_storage = 'system'",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("config.toml");
            fs::write(&path, original).unwrap();
            assert!(ensure_profiles(temp.path()).is_err());
            assert_eq!(fs::read_to_string(path).unwrap(), original);
        }
    }

    #[test]
    fn rejects_shared_custom_storage() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("config.toml"), "[profiles.work]\ndesktop_storage = '/tmp/shared'\n[profiles.personal]\ndesktop_storage = '/tmp/shared/'").unwrap();
        assert!(ensure_profiles(temp.path()).is_err());
    }

    #[test]
    fn persists_interrupted_login_for_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let state = UiState {
            login: Some(LoginSession {
                profile: ProfileId::Work,
                reopen: vec!["personal".into()],
            }),
            ..UiState::default()
        };
        save_state(temp.path(), &state).unwrap();
        let recovered = load_state(temp.path()).unwrap();
        let session = recovered.login.unwrap();
        assert_eq!(session.profile, ProfileId::Work);
        assert_eq!(session.reopen, vec!["personal"]);
        assert!(recovered.initialized.is_empty());
    }

    #[test]
    fn adopts_existing_personal_without_copying_or_deleting_profile_data() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("switch");
        let existing = temp.path().join("existing Claude");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("config.json"), "original settings").unwrap();
        fs::write(existing.join("chat-marker"), "existing chats").unwrap();
        ensure_profiles(&home).unwrap();
        let new_profile = home.join("profiles/personal/desktop");
        fs::create_dir_all(&new_profile).unwrap();
        fs::write(new_profile.join("chat-marker"), "new chats").unwrap();
        let before = fs::read_to_string(home.join("config.toml")).unwrap();
        adopt_personal(&home, &existing).unwrap();
        let doc = fs::read_to_string(home.join("config.toml"))
            .unwrap()
            .parse::<DocumentMut>()
            .unwrap();
        assert_eq!(
            doc["profiles"]["personal"]["desktop_storage"].as_str(),
            Some("system")
        );
        assert_eq!(
            doc["profiles"]["personal"]["code_storage"].as_str(),
            Some("system")
        );
        assert_eq!(
            doc["profiles"]["work"]["desktop_storage"].as_str(),
            Some("isolated")
        );
        assert_eq!(
            fs::read_to_string(existing.join("chat-marker")).unwrap(),
            "existing chats"
        );
        assert_eq!(
            fs::read_to_string(existing.join("config.json")).unwrap(),
            "original settings"
        );
        assert_eq!(
            fs::read_to_string(new_profile.join("chat-marker")).unwrap(),
            "new chats"
        );
        assert_eq!(
            fs::read_to_string(home.join("config.toml.before-existing-personal")).unwrap(),
            before
        );
        assert!(uses_existing_personal(&home).unwrap());
        ensure_profiles(&home).unwrap();
    }

    #[test]
    fn missing_existing_profile_leaves_switch_config_untouched() {
        let temp = tempfile::tempdir().unwrap();
        assert!(adopt_personal(temp.path(), &temp.path().join("absent")).is_err());
        assert!(!temp.path().join("config.toml").exists());
    }

    #[test]
    fn refuses_to_assign_system_storage_already_used_by_another_profile() {
        let temp = tempfile::tempdir().unwrap();
        let existing = temp.path().join("old");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("config.json"), "{}").unwrap();
        ensure_profiles(temp.path()).unwrap();
        let path = temp.path().join("config.toml");
        let raw = fs::read_to_string(&path).unwrap()
            + "\n[profiles.client]\ndesktop_storage = 'system'\n";
        fs::write(&path, &raw).unwrap();
        assert!(adopt_personal(temp.path(), &existing).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), raw);
    }
}
