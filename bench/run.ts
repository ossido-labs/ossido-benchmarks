import os from 'node:os';
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  FRAMEWORKS,
  THREAD_MODES,
  LOAD_SCENARIOS,
  STREAM_SCENARIO,
  OSSIDO_DIR,
  OSSIDO_BINARY,
  NEXT_DIR,
  ROOT,
  cpuCount,
  CONNECTIONS,
  DURATION,
  WARMUP,
  type Framework,
  type ThreadMode,
} from './config.ts';
import { startServer, threadsFor } from './servers.ts';
import { loadTest, streamProbe, type LoadResult, type StreamResult } from './load.ts';
import { writeReport, type BenchReport } from './report.ts';

export interface LoadRecord {
  framework: Framework['key'];
  mode: ThreadMode;
  threads: number;
  scenario: string;
  result: LoadResult;
}

export interface StreamRecord {
  framework: Framework['key'];
  mode: ThreadMode;
  threads: number;
  result: StreamResult;
}

function exec(cmd: string, args: string[], cwd: string, env?: NodeJS.ProcessEnv): Promise<void> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(cmd, args, { cwd, stdio: 'inherit', env: { ...process.env, ...env } });
    child.on('exit', (code) =>
      code === 0 ? resolvePromise() : reject(new Error(`${cmd} ${args.join(' ')} exited ${code}`)),
    );
  });
}

async function buildAll(): Promise<void> {
  console.log('\n=== Building Ossido (JS assets + release binary) ===');
  await exec('bunx', ['ossido', 'build', '--server'], OSSIDO_DIR);
  await exec('cargo', ['build', '--release'], OSSIDO_DIR);
  console.log('\n=== Building Next.js (production) ===');
  await exec('bun', ['run', 'build'], NEXT_DIR);
}

function ensureBuilt(): void {
  const binary = OSSIDO_BINARY;
  const nextDir = resolve(NEXT_DIR, '.next');
  const problems: string[] = [];
  if (!existsSync(binary)) problems.push(`missing Ossido binary: ${binary}`);
  if (!existsSync(nextDir)) problems.push(`missing Next build: ${nextDir}`);
  if (problems.length) {
    console.error('\nApps are not built:\n  - ' + problems.join('\n  - '));
    console.error('\nRun `bun run bench:build` first (or `bun run bench -- --build`).');
    process.exit(1);
  }
}

function url(framework: Framework, path: string): string {
  return `http://127.0.0.1:${framework.port}${path}`;
}

async function main(): Promise<void> {
  const shouldBuild = process.argv.includes('--build');
  if (shouldBuild) await buildAll();
  ensureBuilt();

  const cores = cpuCount();
  console.log('\n=== Ossido vs Next.js benchmark ===');
  console.log(
    `host: ${os.type()} ${os.release()} · ${cores} logical cores · ${(
      os.totalmem() /
      1024 ** 3
    ).toFixed(1)} GB RAM`,
  );
  console.log(
    `load: ${CONNECTIONS} connections · ${DURATION}s measured (+${WARMUP}s warm-up) per scenario\n`,
  );

  const loadRecords: LoadRecord[] = [];
  const streamRecords: StreamRecord[] = [];

  for (const framework of FRAMEWORKS) {
    for (const mode of THREAD_MODES) {
      const label = `${framework.label} · ${mode} (${mode === 'single' ? 1 : cores} thread${
        mode === 'single' ? '' : 's'
      })`;
      process.stdout.write(`\n▶ ${label}\n  starting server… `);
      const server = await startServer(framework, threadsFor(mode));
      console.log(`up (threads=${server.threads})`);
      try {
        for (const scenario of LOAD_SCENARIOS) {
          process.stdout.write(`  ${scenario.title.padEnd(22)} `);
          const result = await loadTest(url(framework, scenario.path), scenario.connections);
          loadRecords.push({
            framework: framework.key,
            mode,
            threads: server.threads,
            scenario: scenario.key,
            result,
          });
          console.log(
            `${Math.round(result.rps).toString().padStart(7)} req/s · ` +
              `p99 ${result.latencyP99.toFixed(1)}ms` +
              (result.errors ? ` · ${result.errors} errors` : ''),
          );
        }
        process.stdout.write(`  ${STREAM_SCENARIO.title.padEnd(22)} `);
        const streamResult = await streamProbe(url(framework, STREAM_SCENARIO.path));
        streamRecords.push({
          framework: framework.key,
          mode,
          threads: server.threads,
          result: streamResult,
        });
        console.log(
          `TTFB ${streamResult.ttfbMs.toFixed(1)}ms · total ${streamResult.totalMs.toFixed(1)}ms`,
        );
      } finally {
        await server.stop();
      }
    }
  }

  const report: BenchReport = {
    generatedAt: new Date().toISOString(),
    cores,
    host: `${os.type()} ${os.release()} · ${os.arch()}`,
    cpuModel: os.cpus()[0]?.model ?? 'unknown',
    totalMemGb: Number((os.totalmem() / 1024 ** 3).toFixed(1)),
    connections: CONNECTIONS,
    durationSec: DURATION,
    loadRecords,
    streamRecords,
  };

  const outPath = resolve(ROOT, 'RESULTS.md');
  await writeReport(report, outPath);
  console.log(`\n✔ Report written to ${outPath}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
