import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const manifest = fileURLToPath(new URL('../upstream.json', import.meta.url));
const root = fileURLToPath(new URL('../..', import.meta.url));
const upstream = JSON.parse(readFileSync(manifest, 'utf8'));
const git = args => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
if (git(['status', '--porcelain'])) throw new Error('Commit or stash local changes before syncing.');
const remote = git(['remote', 'get-url', 'upstream']);
if (remote !== upstream.repository) throw new Error(`Unexpected upstream remote: ${remote}`);
git(['fetch', 'upstream', upstream.branch]);
const revision = git(['rev-parse', 'FETCH_HEAD']);
execFileSync('git', ['merge', '--no-edit', revision], { cwd: root, stdio: 'inherit' });
upstream.revision = revision;
writeFileSync(manifest, `${JSON.stringify(upstream, null, 2)}\n`);
console.log(`CLI synced to ${revision}. Run npm run prepare:cli, npm run test:cli, npm run verify and npm run desktop:build. Review and commit upstream.json after validation.`);
