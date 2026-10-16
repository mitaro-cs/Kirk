// Готовит нативные части перед `tauri dev` / `tauri build`:
// 1. мост Claude Code (crates/kirk-bridge) → src-tauri/binaries/kirk-bridge-<триплет>[.exe]
//    (sidecar Tauri кладёт его рядом с приложением);
// 2. на macOS – mediaremote-adapter, если его исходники лежат в vendor/mediaremote-adapter
//    (git-подмодуль; без него музыка на Mac – через уведомления Music и Spotify).
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { stdio: 'inherit', cwd: root, ...opts });

const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/host: (\S+)/)[1];
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || host;
const release = process.env.TAURI_ENV_DEBUG !== 'true' && process.argv.includes('--release');
const exe = triple.includes('windows') ? '.exe' : '';

const args = ['build', '-p', 'kirk-bridge', '--target', triple];
if (release) args.push('--release');
run('cargo', args);
const built = join(root, 'target', triple, release ? 'release' : 'debug', `kirk-bridge${exe}`);
const binaries = join(root, 'src-tauri', 'binaries');
mkdirSync(binaries, { recursive: true });
copyFileSync(built, join(binaries, `kirk-bridge-${triple}${exe}`));
console.log(`мост: ${triple}`);

if (triple.includes('apple-darwin')) {
  const vendor = join(root, 'vendor', 'mediaremote-adapter');
  const out = join(root, 'src-tauri', 'resources', 'mediaremote');
  const config = join(root, 'src-tauri', 'tauri.macos.conf.json');
  if (existsSync(join(vendor, 'src'))) {
    rmSync(out, { recursive: true, force: true });
    mkdirSync(out, { recursive: true });
    run('sh', [join(root, 'scripts', 'mediaremote.sh'), vendor, out, triple.split('-')[0]]);
    writeFileSync(
      config,
      JSON.stringify(
        {
          bundle: {
            resources: { 'resources/mediaremote/mediaremote-adapter.pl': 'mediaremote-adapter.pl' },
            macOS: { frameworks: ['resources/mediaremote/MediaRemoteAdapter.framework'] },
          },
        },
        null,
        2,
      ) + '\n',
    );
    console.log('mediaremote-adapter: собран');
  } else {
    rmSync(config, { force: true });
    console.log(
      'mediaremote-adapter: нет vendor/mediaremote-adapter – музыка через уведомления Music и Spotify',
    );
  }
}
