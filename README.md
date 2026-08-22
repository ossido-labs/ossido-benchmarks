<div align="center">
  <a href="https://ossido.dev">
    <img src="https://raw.githubusercontent.com/ossido-labs/ossido/main/assets/header.png" alt="Ossido" width="100%">
  </a>

  <h1>Ossido vs Next.js — Benchmark</h1>

  <p><strong>A reproducible benchmark pitting <a href="https://ossido.dev">Ossido</a> (React + Rust/axum, multi-threaded V8 SSR render pool) against <a href="https://nextjs.org">Next.js</a> (React on Node.js).</strong></p>

  <p>
    <a href="https://www.npmjs.com/package/@ossido-labs/ossido"><img src="https://img.shields.io/npm/v/@ossido-labs/ossido?logo=rust&label=ossido&color=E43717" alt="Ossido version"></a>
    <a href="https://www.npmjs.com/package/next"><img src="https://img.shields.io/npm/v/next?logo=nextdotjs&label=next.js&color=000000" alt="Next.js version"></a>
    <a href="https://bun.com"><img src="https://img.shields.io/badge/runtime-Bun-FBF0DF?logo=bun&logoColor=black" alt="Bun"></a>
    <img src="https://img.shields.io/badge/rust-edition%202024-E43717?logo=rust&logoColor=white" alt="Rust edition 2024">
  </p>

  <p>
    <a href="./results">Results</a> ·
    <a href="https://ossido.dev">Ossido docs</a>
  </p>
</div>

---

Both apps live in `examples/` and render **byte-for-byte identical React trees
from identical data**, so the numbers reflect the runtime, not the workload.
Results are written under [`results/<ossido-version>/`](./results) — one folder
per Ossido release — as Markdown (`RESULTS.md`, `MEMORY.md`) plus machine-readable
JSON (`results.json`, `memory.json`).

## Benchmark results

<!-- BENCH_TABLE:START -->
| Ossido version | Next version | Throughput result | Memory result |
| --- | --- | --- | --- |
| `0.1.8-beta.20260822040659Z` | `16.3.2` | [RESULTS.md](./results/0.1.8-beta.20260822040659Z/RESULTS.md) · [json](./results/0.1.8-beta.20260822040659Z/results.json) | [MEMORY.md](./results/0.1.8-beta.20260822040659Z/MEMORY.md) · [json](./results/0.1.8-beta.20260822040659Z/memory.json) |
<!-- BENCH_TABLE:END -->

## What it measures

Four matched routes exist in both apps (`examples/ossido/src/routes/*` and
`examples/next/src/app/*`), sharing the identical components in each app's
`src/bench/` directory:

| Route        | Scenario                                       | What it stresses                             |
|--------------|------------------------------------------------|----------------------------------------------|
| `/ssr`       | 60 product cards, rendered per request         | Typical SSR page                             |
| `/heavy`     | 5000-row table, rendered per request           | CPU-bound SSR (where a render pool pays off) |
| `/stream`    | Shell first, then a 3000-row table streamed in | Streaming SSR / time-to-first-byte           |
| `/api/bench` | 100-item JSON payload                          | Backend request path (Rust/axum vs Node)     |

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

# Run the benchmark → results/<ossido-version>/{RESULTS.md,results.json}
bun run bench

# …or build and run in one step
bun run bench:all

# Quick, low-fidelity smoke run (print-only — writes no output files)
bun run bench:quick

# Memory-efficiency sweep → results/<ossido-version>/{MEMORY.md,memory.json}
bun run bench:memory
```

### Output files

Each run writes a human-readable Markdown report **and** a machine-readable JSON
sibling (both committed), under `results/<ossido-version>/` so every Ossido
release keeps its own results:

```
results/
  <ossido-version>/
    RESULTS.md   results.json    # throughput / latency / streaming (bun run bench)
    MEMORY.md    memory.json     # memory efficiency (bun run bench:memory)
```

The JSON is self-describing (`schema` field — `ossido-benchmark/results@1` /
`ossido-benchmark/memory@1`) and carries the environment, framework versions,
load parameters, and one record per (framework × config × scenario).

## Memory efficiency

`bun run bench:memory` tests a specific hypothesis: **Ossido serves far more
requests per unit of memory.** Ossido scales SSR across cores with V8 render
threads inside a *single* Rust process (one shared heap), whereas Next.js scales
by forking *N* full Node.js processes (one heap each).

The sweep runs identical `/ssr` load at each parallelism level (1, 2, 4, …,
cores), samples the resident memory (RSS) of the **entire server process group**
every 250ms during the load, and reports **req/s per MB**. Results, tables and
charts land in the version's `MEMORY.md` under [`results/`](./results).

### Tuning

Environment variables (all optional) control the load:

| Variable             | Default | Meaning                                  |
|----------------------|---------|------------------------------------------|
| `BENCH_DURATION`     | `10`    | Measured seconds per scenario            |
| `BENCH_WARMUP`       | `3`     | Warm-up seconds (discarded) per scenario |
| `BENCH_CONNECTIONS`  | `50`    | Concurrent connections                   |
| `BENCH_PIPELINING`   | `1`     | Requests pipelined per connection        |
| `BENCH_TTFB_SAMPLES` | `30`    | Samples for the streaming TTFB probe     |

Example: `BENCH_DURATION=20 BENCH_CONNECTIONS=100 bun run bench`.

> **Setting any of these makes the run print-only.** The committed
> `results/<version>/` files and the README table are only written by a
> **default-configuration** run, so an ad-hoc/tuned run never overwrites the
> canonical, reproducible results.

## How it works

```
bench/
  config.ts    ports, scenarios, thread modes, load parameters
  build.ts     production builds for both apps
  servers.ts   start/stop each server (Ossido binary; Next cluster server)
  load.ts      autocannon load test + streaming TTFB probe
  report.ts    renders RESULTS.md + results.json (tables + mermaid charts)
  run.ts       orchestrator → results/<ossido-version>/{RESULTS.md,results.json}
  memory.ts    memory sweep → results/<ossido-version>/{MEMORY.md,memory.json}
  index.ts     rebuilds the results table in this README (bun run bench:index)
examples/
  ossido/      Ossido app (Rust page handlers + React routes)
  next/        Next.js app (App Router); server.mjs is the clustered prod server
results/
  <ossido-version>/  generated reports (Markdown + JSON), one folder per release
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
