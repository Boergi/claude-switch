import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, chmodSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../src-tauri/resources/cswitch', import.meta.url));
const sandbox = mkdtempSync(path.join(tmpdir(), 'claude-switch-contract-'));
const env = { ...process.env, CSWITCH_HOME: sandbox, NO_COLOR: '1', ANTHROPIC_API_KEY: 'must-be-stripped', CLAUDE_CODE_OAUTH_TOKEN: 'must-be-stripped' };
const run = args => execFileSync(cli, args, { env, encoding: 'utf8', timeout: 30000 });
try {
  assert.match(run(['desktop', '--help']), /profile/);
  assert.match(run(['login', '--help']), /--no-wait/);
  const fake = path.join(sandbox, 'Fake Claude');
  const observations = path.join(sandbox, 'launches.jsonl');
  const script = `#!${process.execPath}\nrequire('node:fs').appendFileSync(${JSON.stringify(observations)}, JSON.stringify({ args: process.argv.slice(2), profile: process.env.CSWITCH_PROFILE, config: process.env.CLAUDE_CONFIG_DIR, apiKey: process.env.ANTHROPIC_API_KEY, token: process.env.CLAUDE_CODE_OAUTH_TOKEN }) + '\\n');\n`;
  writeFileSync(fake, script);
  chmodSync(fake, 0o755);
  const tomlPath = JSON.stringify(fake);
  writeFileSync(path.join(sandbox, 'config.toml'), `[desktop]\nexe_path = ${tomlPath}\nrouter_enabled = false\n[profiles.work]\nlabel = 'Arbeit'\n[profiles.personal]\nlabel = 'Privat'\n`);
  const status = JSON.parse(run(['status', '--json']));
  assert.ok(status.desktop_install);
  assert.equal(status.profiles.length, 2);
  const work = status.profiles.find(p => p.name === 'work');
  const personal = status.profiles.find(p => p.name === 'personal');
  assert.equal(work.label, 'Arbeit');
  assert.equal(personal.label, 'Privat');
  assert.notEqual(work.desktop.data_dir, personal.desktop.data_dir);
  assert.ok(work.desktop.data_dir.startsWith(sandbox));
  assert.ok(personal.desktop.data_dir.startsWith(sandbox));
  for (const profile of ['work', 'personal']) run(['desktop', profile]);
  let launches = [];
  for (let attempt = 0; attempt < 40; attempt++) {
    if (existsSync(observations)) launches = readFileSync(observations, 'utf8').trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
    if (launches.length === 2) break;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  assert.equal(launches.length, 2, 'both launches must reach the fake desktop');
  for (const profile of ['work', 'personal']) {
    const launch = launches.find(p => p.profile === profile);
    assert.ok(launch, `missing ${profile} launch`);
    assert.deepEqual(launch.args, [`--user-data-dir=${path.join(sandbox, 'profiles', profile, 'desktop')}`]);
    assert.equal(launch.apiKey, undefined);
    assert.equal(launch.token, undefined);
    assert.equal(launch.config, path.join(sandbox, 'profiles', profile, 'code'));
  }
  console.log('CLI_CONTRACT_VERIFIED');
} finally {
  rmSync(sandbox, { recursive: true, force: true });
}
