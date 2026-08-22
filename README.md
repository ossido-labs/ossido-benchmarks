# Ossido vs Next.js benchmark

A reproducible performance benchmark comparing [Ossido](https://ossido.dev)
(React + Rust/axum, with a multi-threaded V8 SSR render pool) against
[Next.js](https://nextjs.org) (React on Node.js). Both apps live in `examples/`
and render **byte-for-byte identical React trees from identical data**, so the
numbers reflect the runtime, not the workload.

The latest results are written to [`RESULTS.md`](./RESULTS.md).

## What it measures

Four matched routes exist in both apps (`examples/ossido/src/routes/*` and
`examples/next/src/app/*`), sharing the identical components in each app's
`src/bench/` directory:

| Route | Scenario | What it stresses |
| --- | --- | --- |
| `/ssr` | 60 product cards, rendered per request | Typical SSR page |
| `/heavy` | 5000-row table, rendered per request | CPU-bound SSR (where a render pool pays off) |
| `/stream` | Shell first, then a 3000-row table streamed in | Streaming SSR / time-to-first-byte |
| `/api/bench` | 100-item JSON payload | Backend request path (Rust/axum vs Node) |

Each is run in two configurations:

- **Single-threaded** — Ossido with `OSSIDO_SSR_THREADS=1`; Next.js as a single
  Node process.
- **Multi-threaded** — Ossido with `OSSIDO_SSR_THREADS=<cores>` (one warm V8
  render isolate per core); Next.js as a `<cores>`-worker Node `cluster` sharing
  one port (the idiomatic way to use multiple cores on Node).

For each, the harness records throughput (req/s), latency (p50/p99), throughput
(MB/s) and errors; the streaming route is probed separately for time-to-first-
byte vs full response.

## Running it

Requirements: [Bun](https://bun.com), a Rust toolchain (`cargo`), and Node.js
(used to run the Next.js production server).

```bash
bun install

# Build both apps (Ossido: JS assets + `cargo build --release`; Next: `next build`)
bun run bench:build

# Run the benchmark and (re)generate RESULTS.md
bun run bench

# …or build and run in one step
bun run bench:all

# Quick, low-fidelity smoke run (short duration, fewer connections)
bun run bench:quick

# Memory-efficiency sweep → writes MEMORY.md
bun run bench:memory
```

## Memory efficiency

`bun run bench:memory` tests a specific hypothesis: **Ossido serves far more
requests per unit of memory.** Ossido scales SSR across cores with V8 render
threads inside a *single* Rust process (one shared heap), whereas Next.js scales
by forking *N* full Node.js processes (one heap each).

The sweep runs identical `/ssr` load at each parallelism level (1, 2, 4, …,
cores), samples the resident memory (RSS) of the **entire server process group**
every 250ms during the load, and reports **req/s per MB**. Results, tables and
charts land in [`MEMORY.md`](./MEMORY.md).

### Tuning

Environment variables (all optional) control the load:

| Variable | Default | Meaning |
| --- | --- | --- |
| `BENCH_DURATION` | `10` | Measured seconds per scenario |
| `BENCH_WARMUP` | `3` | Warm-up seconds (discarded) per scenario |
| `BENCH_CONNECTIONS` | `50` | Concurrent connections |
| `BENCH_PIPELINING` | `1` | Requests pipelined per connection |
| `BENCH_TTFB_SAMPLES` | `30` | Samples for the streaming TTFB probe |

Example: `BENCH_DURATION=20 BENCH_CONNECTIONS=100 bun run bench`.

## How it works

```
bench/
  config.ts    ports, scenarios, thread modes, load parameters
  build.ts     production builds for both apps
  servers.ts   start/stop each server (Ossido binary; Next cluster server)
  load.ts      autocannon load test + streaming TTFB probe
  report.ts    renders RESULTS.md (tables + mermaid charts)
  run.ts       orchestrator (throughput/latency → RESULTS.md)
  memory.ts    memory-efficiency sweep (req/s per MB → MEMORY.md)
examples/
  ossido/      Ossido app (Rust page handlers + React routes)
  next/        Next.js app (App Router); server.mjs is the clustered prod server
```

Ports are kept off `:3000` (Ossido on `:4000`, Next.js on `:4100`) so a running
`ossido dev` won't collide with a benchmark run.

## Fairness notes

- Both apps are **production builds** and render on **every request**
  (Ossido routes carry a trivial `page.rs` handler → `ƒ` dynamic; Next routes
  use `export const dynamic = 'force-dynamic'`), so neither serves cached HTML.
- The data-generation and React components under `src/bench/` are duplicated
  **identically** between the two apps.
- Next.js is scaled across cores with the Node `cluster` module via a documented
  custom server (`examples/next/server.mjs`) — Node cannot use multiple cores for
  SSR within a single process, so this is the standard multi-core approach.
- Results are hardware-dependent; regenerate them on your own machine.
