import { spawn } from 'node:child_process';
import { OSSIDO_DIR, NEXT_DIR } from './config.ts';

function exec(cmd: string, args: string[], cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, stdio: 'inherit' });
    child.on('exit', (code) =>
      code === 0 ? resolve() : reject(new Error(`${cmd} ${args.join(' ')} exited ${code}`)),
    );
  });
}

// Production builds for both apps. Ossido = JS assets (ossido build) + the
// Rust/axum server binary (cargo build --release, LTO). Next.js = next build.
async function main(): Promise<void> {
  console.log('=== Building Ossido: JS assets ===');
  await exec('bunx', ['ossido', 'build', '--server'], OSSIDO_DIR);
  console.log('\n=== Building Ossido: release binary (cargo, LTO — slow the first time) ===');
  await exec('cargo', ['build', '--release'], OSSIDO_DIR);
  console.log('\n=== Building Next.js: production ===');
  await exec('bun', ['run', 'build'], NEXT_DIR);
  console.log('\n✔ Both apps built.');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
