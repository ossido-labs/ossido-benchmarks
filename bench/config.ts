import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
export const ROOT = resolve(__dirname, '..');
export const OSSIDO_DIR = resolve(ROOT, 'examples/ossido');
export const NEXT_DIR = resolve(ROOT, 'examples/next');
// examples/ossido is a member of the root Cargo workspace, so the release
// binary is emitted to the workspace target dir at the repo root.
export const OSSIDO_BINARY = resolve(ROOT, 'target/release/ossido');

export function cpuCount(): number {
  const fn = (os as { availableParallelism?: () => number }).availableParallelism;
  return typeof fn === 'function' ? fn() : os.cpus().length;
}

// Ports. Ossido's port is baked into ossido.config.ts at build time; Next's is
// passed to the custom server via env. Kept off :3000 so a running `ossido dev`
// doesn't collide with the benchmark.
export const OSSIDO_PORT = 4000;
export const NEXT_PORT = 4100;

// Load parameters (override via env for a quick smoke run, e.g.
// BENCH_DURATION=3 BENCH_CONNECTIONS=20 bun run bench).
export const DURATION = Number(process.env.BENCH_DURATION ?? 10); // seconds, measured
export const WARMUP = Number(process.env.BENCH_WARMUP ?? 3); // seconds, discarded
export const CONNECTIONS = Number(process.env.BENCH_CONNECTIONS ?? 50);
export const PIPELINING = Number(process.env.BENCH_PIPELINING ?? 1);
export const TTFB_SAMPLES = Number(process.env.BENCH_TTFB_SAMPLES ?? 30);

export type ThreadMode = 'single' | 'multi';

export interface Framework {
  key: 'ossido' | 'next';
  label: string;
  port: number;
}

export const OSSIDO: Framework = { key: 'ossido', label: 'Ossido', port: OSSIDO_PORT };
export const NEXT: Framework = { key: 'next', label: 'Next.js', port: NEXT_PORT };
export const FRAMEWORKS: Framework[] = [OSSIDO, NEXT];

export interface Scenario {
  key: string;
  title: string;
  path: string;
  note: string;
  // Concurrent connections for this scenario. Defaults to CONNECTIONS. The
  // heavy render is deliberately run at ~= core count: a single ~300ms render
  // saturates a thread on its own, so piling on 50 connections just builds a
  // huge queue (and can exceed the request timeout), without measuring more.
  connections?: number;
}

// Load-tested scenarios (throughput + latency). Paths are identical in both
// apps and render byte-for-byte identical React trees.
export const LOAD_SCENARIOS: Scenario[] = [
  {
    key: 'ssr',
    title: 'SSR — catalogue',
    path: '/ssr',
    note: '60 product cards rendered per request',
  },
  {
    key: 'heavy',
    title: 'SSR — heavy table',
    path: '/heavy',
    note: '5000-row table, CPU-bound render',
    connections: cpuCount() * 2,
  },
  {
    key: 'api',
    title: 'JSON API',
    path: '/api/bench',
    note: '100-item JSON payload (Rust vs Node)',
  },
];

// Measured separately for time-to-first-byte vs full response.
export const STREAM_SCENARIO: Scenario = {
  key: 'stream',
  title: 'Streaming SSR',
  path: '/stream',
  note: 'shell flushed first, 3000-row table streamed after',
};

export const THREAD_MODES: ThreadMode[] = ['single', 'multi'];
