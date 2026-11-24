import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { App } from './App';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));
const call = vi.mocked(invoke);
const ready = { installed: true, login: null, profiles: [
  { id: 'work', running: false, initialized: true },
  { id: 'personal', running: false, initialized: true },
] };

beforeEach(() => { call.mockReset(); call.mockResolvedValue(ready); });

describe('Desktop launcher', () => {
  it('opens each initialized account with its own fixed profile argument', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Claude Arbeit öffnen' })).toBeEnabled());
    await user.click(screen.getByRole('button', { name: 'Claude Arbeit öffnen' }));
    expect(call).toHaveBeenCalledWith('open_profile', { profile: 'work' });
    await user.click(screen.getByRole('button', { name: 'Claude Privat öffnen' }));
    expect(call).toHaveBeenCalledWith('open_profile', { profile: 'personal' });
  });

  it('requires explicit login confirmation on first use', async () => {
    call.mockResolvedValue({ ...ready, profiles: ready.profiles.map(p => ({ ...p, initialized: false })) });
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Claude Arbeit öffnen' })).toBeEnabled());
    await user.click(screen.getByRole('button', { name: 'Claude Arbeit öffnen' }));
    expect(screen.getByRole('dialog')).toBeVisible();
    expect(call).not.toHaveBeenCalledWith('begin_login', expect.anything());
    call.mockResolvedValue({ ...ready, login: 'work' });
    await user.click(screen.getByRole('button', { name: 'Profil zur Anmeldung öffnen' }));
    expect(call).toHaveBeenCalledWith('begin_login', { profile: 'work' });
    call.mockResolvedValue(ready);
    await user.click(screen.getByRole('button', { name: 'Ich bin angemeldet' }));
    expect(call).toHaveBeenCalledWith('finish_login', { completed: true });
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('recovers an interrupted login and restores profiles on cancellation', async () => {
    call.mockResolvedValue({ ...ready, login: 'personal' });
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole('button', { name: 'Ich bin angemeldet' })).toBeVisible();
    call.mockResolvedValue(ready);
    await user.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(call).toHaveBeenCalledWith('finish_login', { completed: false });
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('prevents duplicate launches while a command is pending', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Claude Arbeit öffnen' })).toBeEnabled());
    let resolveLaunch: (value: typeof ready) => void = () => {};
    call.mockImplementation(command => command === 'open_profile'
      ? new Promise(resolve => { resolveLaunch = resolve; }) : Promise.resolve(ready));
    await user.click(screen.getByRole('button', { name: 'Claude Arbeit öffnen' }));
    expect(screen.getByRole('button', { name: 'Claude Privat öffnen' })).toBeDisabled();
    resolveLaunch(ready);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Claude Privat öffnen' })).toBeEnabled());
  });

  it('shows launch errors and allows retry', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Claude Arbeit öffnen' })).toBeEnabled());
    call.mockImplementation(command => command === 'open_profile'
      ? Promise.reject('Claude konnte nicht gestartet werden.') : Promise.resolve(ready));
    await user.click(screen.getByRole('button', { name: 'Claude Arbeit öffnen' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Claude konnte nicht gestartet werden.');
    expect(screen.getByRole('button', { name: 'Claude Arbeit öffnen' })).toBeEnabled();
  });

  it('blocks launch when Claude Desktop is absent', async () => {
    call.mockResolvedValue({ ...ready, installed: false });
    render(<App />);
    expect(await screen.findByText('Installiere zuerst Claude Desktop in deinem Programme-Ordner.')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Claude Arbeit öffnen' })).toBeDisabled();
  });

  it('adopts an existing personal profile only after confirmation', async () => {
    call.mockResolvedValue({ ...ready, can_adopt_personal: true });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Bisheriges Privatprofil verwenden' }));
    expect(screen.getByRole('dialog')).toBeVisible();
    expect(call).not.toHaveBeenCalledWith('adopt_personal', expect.anything());
    call.mockResolvedValue({ ...ready, can_adopt_personal: false });
    await user.click(screen.getByRole('button', { name: 'Bisheriges Profil verwenden' }));
    expect(call).toHaveBeenCalledWith('adopt_personal', {});
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(screen.getByText('Dein bisheriges Claude ist jetzt mit „Privat“ verbunden.')).toBeVisible();
  });
});
