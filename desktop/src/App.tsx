import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';

type ProfileId = 'work' | 'personal';
type ProfileStatus = { id: ProfileId; running: boolean; initialized: boolean };
type AppStatus = { installed: boolean; profiles: ProfileStatus[]; login: ProfileId | null; can_adopt_personal: boolean };

const profiles = [
  { id: 'work' as const, title: 'Arbeit', description: 'Raum für deine Arbeit.', initial: 'A' },
  { id: 'personal' as const, title: 'Privat', description: 'Raum für deine eigenen Ideen.', initial: 'P' },
];

function Arrow() {
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden="true"><path d="M5 12h14m-6-6 6 6-6 6" /></svg>;
}

export function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [selected, setSelected] = useState<ProfileId | null>(null);
  const [adopting, setAdopting] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const trigger = useRef<HTMLButtonElement | null>(null);

  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    try {
      const current = await invoke<AppStatus>('app_status');
      setStatus(current);
      if (current.login) setSelected(current.login);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => {
    if (busy) return;
    const onFocus = () => { void refresh(); };
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [busy, refresh]);

  useEffect(() => {
    if (selected && !dialog.current?.open) dialog.current?.showModal();
    if (!selected && dialog.current?.open) {
      dialog.current.close();
      trigger.current?.focus();
    }
  }, [selected]);

  async function action(command: string, args: Record<string, unknown>, after?: () => void) {
    setBusy(true);
    setError('');
    setNotice('');
    try {
      setStatus(await invoke<AppStatus>(command, args));
      after?.();
    } catch (err) {
      setError(String(err));
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  function openProfile(id: ProfileId, button: HTMLButtonElement) {
    trigger.current = button;
    if (!status?.profiles.find(p => p.id === id)?.initialized) {
      setError('');
      setSelected(id);
      return;
    }
    void action('open_profile', { profile: id }, () => setNotice(`${id === 'work' ? 'Arbeit' : 'Privat'} wurde geöffnet.`));
  }

  function closeDialog() {
    if (busy) return;
    if (status?.login) {
      void action('finish_login', { completed: false }, () => setSelected(null));
    } else {
      setSelected(null);
      setAdopting(false);
      setError('');
    }
  }

  const preview = !isTauri();
  const selectedTitle = profiles.find(p => p.id === selected)?.title;
  const loginActive = Boolean(status?.login);
  const unavailable = !preview && (!status || !status.installed);

  return (
    <main>
      <header className="app-header">
        <div className="app-mark" aria-hidden="true"><svg width="27" height="27" viewBox="0 0 32 32" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M8 11h17l-5-5M24 21H7l5 5" /></svg></div>
        <span>Claude Switch</span>
        <span className="local-label"><span aria-hidden="true">◦</span> Nur auf deinem Mac</span>
      </header>

      <section className="intro" aria-labelledby="title">
        <p className="eyebrow">Zwei Profile. Dein Claude.</p>
        <h1 id="title">Was steht heute an?</h1>
        <p>Öffne Claude Desktop mit dem passenden Konto.</p>
      </section>

      <section className="profiles" aria-label="Claude-Profile" aria-busy={busy}>
        {profiles.map(profile => {
          const current = status?.profiles.find(p => p.id === profile.id);
          return <article key={profile.id} className={`profile ${profile.id}`}>
            <div className="profile-heading">
              <span className="avatar" aria-hidden="true">{profile.initial}</span>
              <span className="profile-state"><span className={current?.running ? 'dot running' : 'dot'} />{current?.running ? 'Geöffnet' : current?.initialized ? 'Eingerichtet' : 'Erste Anmeldung'}</span>
            </div>
            <h2>{profile.title}</h2>
            <p>{profile.description}</p>
            <button className="launch" disabled={busy || unavailable || preview} onClick={event => openProfile(profile.id, event.currentTarget)} aria-label={`Claude ${profile.title} öffnen`}>
              <span>{busy && selected === null ? 'Einen Moment …' : 'Claude öffnen'}</span><Arrow />
            </button>
          </article>;
        })}
      </section>

      {status?.can_adopt_personal && <button className="adopt-button text-button" disabled={busy} onClick={event => {
        trigger.current = event.currentTarget;
        setError('');
        setAdopting(true);
        setSelected('personal');
      }}>Bisheriges Privatprofil verwenden</button>}

      <div className="feedback" aria-live="polite">
        {!selected && error && <p className="error" role="alert">{error} <button className="text-button" disabled={busy} onClick={() => { setError(''); void refresh(); }}>Erneut versuchen</button></p>}
        {!error && notice && <p>{notice}</p>}
        {!error && !notice && unavailable && <p>{status ? 'Installiere zuerst Claude Desktop in deinem Programme-Ordner.' : 'Claude Desktop wird gesucht …'}</p>}
        {preview && <p>Oberflächenvorschau. Profile öffnest du in der macOS-App.</p>}
      </div>

      <footer><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><rect x="5" y="10" width="14" height="11" rx="2" /><path d="M8 10V7a4 4 0 0 1 8 0v3" /></svg><span>Getrennte Konten. Beide bleiben angemeldet.</span></footer>

      <dialog ref={dialog} aria-labelledby="login-title" aria-describedby="login-description" onCancel={event => { event.preventDefault(); closeDialog(); }}>
        <div className="dialog-content">
          <p className="eyebrow">{selectedTitle}</p>
          <h2 id="login-title">{adopting ? 'Dein bisheriges Claude behalten.' : loginActive ? 'Jetzt in Claude anmelden.' : 'Einmal anmelden. Dann loslegen.'}</h2>
          <p id="login-description">{adopting ? '„Privat“ verwendet danach den Datenordner deiner normalen Claude-App. Deine bisherigen Chats, Einstellungen und deine Anmeldung bleiben dort. Es wird nichts verschoben oder gelöscht.' : loginActive
            ? `Melde dich im geöffneten Claude-Fenster mit deinem ${selected === 'work' ? 'Arbeitskonto' : 'privaten Konto'} an. Nutze ein privates Browserfenster, falls dein Browser schon mit einem anderen Konto angemeldet ist.`
            : 'Wir richten zwei getrennte Profile ein. Deine bisherige Claude-Installation und ihre Daten bleiben erhalten.'}</p>
          <div className="login-note">{adopting ? 'Beende vorher das neu angelegte private Claude-Fenster mit Cmd+Q. Der neue Profilordner bleibt als Rückfallmöglichkeit erhalten. Wir sichern die Switch-Konfiguration vor dem Wechsel.' : loginActive
            ? 'Wenn du deine Chats in Claude siehst, bestätige unten. Zuvor geöffnete Switch-Profile werden wieder geöffnet.'
            : 'Für die Anmeldung schließt das CLI andere Claude-Fenster vorübergehend. Beende laufende Aufgaben vorher. Normale Claude-Fenster kannst du danach selbst wieder öffnen.'}</div>
          {error && <p className="error" role="alert">{error}</p>}
          <div className="dialog-actions">
            <button className="secondary" disabled={busy} onClick={closeDialog}>{loginActive ? 'Abbrechen' : 'Später'}</button>
            <button className="primary" disabled={busy} onClick={() => {
              if (adopting) void action('adopt_personal', {}, () => { setSelected(null); setAdopting(false); setNotice('Dein bisheriges Claude ist jetzt mit „Privat“ verbunden.'); });
              else if (loginActive) void action('finish_login', { completed: true }, () => { setSelected(null); setNotice(`${selectedTitle} ist eingerichtet.`); });
              else void action('begin_login', { profile: selected });
            }}>{busy ? 'Einen Moment …' : adopting ? 'Bisheriges Profil verwenden' : loginActive ? 'Ich bin angemeldet' : 'Profil zur Anmeldung öffnen'}</button>
          </div>
        </div>
      </dialog>
    </main>
  );
}
