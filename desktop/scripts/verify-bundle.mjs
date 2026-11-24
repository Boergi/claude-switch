import assert from 'node:assert/strict';
import { accessSync, constants, readFileSync, readdirSync, statSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const target = fileURLToPath(new URL('../src-tauri/target/release/bundle/', import.meta.url));
const contents = path.join(target, 'macos/Claude Switch.app/Contents');
accessSync(path.join(contents, 'MacOS/claude-switch-desktop'), constants.X_OK);
accessSync(path.join(contents, 'Resources/cswitch'), constants.X_OK);
assert.ok(statSync(path.join(contents, 'Resources/CLI-LICENSE')).size > 0);
assert.match(readFileSync(path.join(contents, 'Info.plist'), 'utf8'), /dev.boergi.claudeswitch/);
assert.match(execFileSync(path.join(contents, 'Resources/cswitch'), ['--version'], { encoding: 'utf8' }), /^cswitch \d+\.\d+\.\d+/);
const dmg = readdirSync(path.join(target, 'dmg')).find(name => name.endsWith('.dmg'));
assert.ok(dmg, 'installable disk image is missing');
assert.ok(statSync(path.join(target, 'dmg', dmg)).size > 0);
execFileSync('/usr/bin/codesign', ['--verify', '--deep', '--strict', path.join(target, 'macos/Claude Switch.app')], { stdio: 'inherit' });
console.log('MACOS_BUNDLE_VERIFIED');
