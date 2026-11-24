import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const sandbox = mkdtempSync(path.join(tmpdir(), 'claude-switch-sync-'));
const original = path.join(sandbox, 'original');
const fork = path.join(sandbox, 'fork');
const git = (cwd, ...args) => execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
const identify = cwd => {
  git(cwd, 'config', 'user.name', 'Sync test');
  git(cwd, 'config', 'user.email', 'test@example.invalid');
  git(cwd, 'config', 'commit.gpgsign', 'false');
};
try {
  mkdirSync(original);
  git(original, 'init', '--initial-branch=master');
  identify(original);
  writeFileSync(path.join(original, 'cli.txt'), 'original CLI\n');
  git(original, 'add', '.');
  git(original, 'commit', '-m', 'Initial CLI');
  const first = git(original, 'rev-parse', 'HEAD');
  git(sandbox, 'clone', original, fork);
  identify(fork);
  git(fork, 'remote', 'rename', 'origin', 'upstream');
  mkdirSync(path.join(fork, 'desktop/scripts'), { recursive: true });
  copyFileSync(fileURLToPath(new URL('./sync-cli.mjs', import.meta.url)), path.join(fork, 'desktop/scripts/sync-cli.mjs'));
  writeFileSync(path.join(fork, 'desktop/upstream.json'), JSON.stringify({ repository: original, branch: 'master', revision: first }));
  writeFileSync(path.join(fork, 'desktop/ui.txt'), 'two profile buttons\n');
  git(fork, 'add', '.');
  git(fork, 'commit', '-m', 'Add desktop');
  writeFileSync(path.join(original, 'cli.txt'), 'updated CLI\n');
  git(original, 'commit', '-am', 'Update CLI');
  const next = git(original, 'rev-parse', 'HEAD');
  execFileSync(process.execPath, [path.join(fork, 'desktop/scripts/sync-cli.mjs')], { cwd: fork, stdio: 'pipe' });
  assert.equal(readFileSync(path.join(fork, 'cli.txt'), 'utf8'), 'updated CLI\n');
  assert.equal(readFileSync(path.join(fork, 'desktop/ui.txt'), 'utf8'), 'two profile buttons\n');
  assert.equal(JSON.parse(readFileSync(path.join(fork, 'desktop/upstream.json'), 'utf8')).revision, next);
  assert.equal(git(fork, 'diff', next, '--', 'cli.txt'), '');
  const dirty = spawnSync(process.execPath, [path.join(fork, 'desktop/scripts/sync-cli.mjs')], { cwd: fork, encoding: 'utf8' });
  assert.notEqual(dirty.status, 0);
  assert.match(dirty.stderr, /Commit or stash local changes/);
  console.log('UPSTREAM_SYNC_VERIFIED');
} finally {
  rmSync(sandbox, { recursive: true, force: true });
}
