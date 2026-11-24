import { execFileSync } from 'node:child_process';
import { chmodSync, copyFileSync, mkdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const desktop = fileURLToPath(new URL('..', import.meta.url));
const root = path.dirname(desktop);
const upstream = JSON.parse(readFileSync(path.join(desktop, 'upstream.json'), 'utf8'));
execFileSync('git', ['diff', '--exit-code', upstream.revision, '--', 'src', 'Cargo.toml', 'Cargo.lock'], { cwd: root, stdio: 'inherit' });
execFileSync('cargo', ['build', '--release', '--locked', '--manifest-path', path.join(root, 'Cargo.toml')], { cwd: root, stdio: 'inherit' });
const resources = path.join(desktop, 'src-tauri/resources');
mkdirSync(resources, { recursive: true });
copyFileSync(path.join(root, 'target/release/cswitch'), path.join(resources, 'cswitch'));
chmodSync(path.join(resources, 'cswitch'), 0o755);
console.log(`Bundled unchanged CLI from ${upstream.revision}`);
