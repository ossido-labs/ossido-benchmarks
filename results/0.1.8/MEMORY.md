# Ossido vs Next.js — memory efficiency

> Hypothesis: **Ossido serves far more requests per unit of memory.**
>
> Ossido scales SSR across cores with V8 render threads inside a *single*
> Rust process (one shared heap); Next.js scales by forking *N* full Node.js
> processes (one heap each). This sweep runs the same `/ssr` load at each
> parallelism level while sampling the resident memory (RSS) of the whole
> server process group, and reports **req/s per MB**.

## Environment

| | |
| --- | --- |
| Date | 2026-08-23T02:30:04.765602Z |
| Host | Darwin 25.5.0 · aarch64 |
| CPU | Apple M4 Max |
| Logical cores | 16 |
| Memory | 48.0 GB |
| Load | 50 connections, 10s (+3s warm-up), route `/ssr` |

## Results

| Framework | Parallelism | Idle RSS (MB) | Mean RSS (MB) | Peak RSS (MB) | req/s | **req/s per MB** |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Ossido | 1 | 35 | 375 | 541 | 2,052 | **5.5** |
| Ossido | 2 | 40 | 667 | 912 | 1,070 | **1.6** |
| Ossido | 4 | 50 | 1,133 | 1,622 | 5,828 | **5.1** |
| Ossido | 8 | 69 | 2,000 | 2,778 | 9,101 | **4.6** |
| Ossido | 16 | 107 | 2,928 | 4,008 | 9,135 | **3.1** |
| Next.js | 1 | 117 | 516 | 550 | 576 | **1.1** |
| Next.js | 2 | 294 | 956 | 994 | 1,124 | **1.2** |
| Next.js | 4 | 512 | 1,802 | 1,912 | 2,161 | **1.2** |
| Next.js | 8 | 943 | 3,493 | 3,686 | 3,666 | **1.0** |
| Next.js | 16 | 1,813 | 6,148 | 6,845 | 3,983 | **0.6** |

## Headline

At full parallelism (16 threads / workers), serving `/ssr`:

- **Ossido:** 9,135 req/s using 2,928 MB → **3.1 req/s per MB**
- **Next.js:** 3,983 req/s using 6,148 MB → **0.6 req/s per MB**

➡️ Ossido is **4.8× more memory-efficient** here, and uses **2.1× less RAM** (2,928 MB vs 6,148 MB) while serving 2.29× the throughput.

**Baseline footprint:** the gap is starkest at idle. Scaling Ossido from 1 to 16 threads adds only **72 MB** (shared process, 107 MB total); scaling Next.js to 16 workers costs **1,813 MB** — **17× more** just to sit idle, because every worker is a full Node.js + Next.js heap.

**Iso-memory:** within Ossido's 2,928 MB footprint, Next.js can only run 4 worker(s) (1,802 MB, 2,161 req/s) — Ossido serves 4.2× the requests in the same RAM.

## Charts

*Bars are the parallelism sweep: `Oss xn` = Ossido with n threads, `Next xn` =
Next.js with n workers.*

### Throughput vs parallelism (req/s — higher is better)

```mermaid
xychart-beta
    title "Throughput — req/s"
    x-axis ["Oss x1", "Oss x2", "Oss x4", "Oss x8", "Oss x16", "Next x1", "Next x2", "Next x4", "Next x8", "Next x16"]
    y-axis "req/s"
    bar [2051.74, 1070.38, 5827.58, 9101.26, 9135.12, 575.62, 1123.89, 2161.01, 3665.77, 3982.59]
```

### Memory vs parallelism (mean RSS, MB — lower is better)

```mermaid
xychart-beta
    title "Memory — mean RSS (MB)"
    x-axis ["Oss x1", "Oss x2", "Oss x4", "Oss x8", "Oss x16", "Next x1", "Next x2", "Next x4", "Next x8", "Next x16"]
    y-axis "MB"
    bar [375.26, 667.08, 1132.52, 2000.12, 2928.35, 516.27, 956.1, 1802.36, 3493.02, 6148.07]
```

### Efficiency vs parallelism (req/s per MB — higher is better)

This is the direct test of the hypothesis.

```mermaid
xychart-beta
    title "Efficiency — req/s per MB"
    x-axis ["Oss x1", "Oss x2", "Oss x4", "Oss x8", "Oss x16", "Next x1", "Next x2", "Next x4", "Next x8", "Next x16"]
    y-axis "req/s per MB"
    bar [5.47, 1.6, 5.15, 4.55, 3.12, 1.11, 1.18, 1.2, 1.05, 0.65]
```

---

### How this is measured

- **Memory:** RSS of the entire server process group (Ossido: one process
  with N render threads; Next.js: the cluster primary + N workers), summed
  and sampled every 250ms during the load. *Idle* is captured just
  before load; *mean*/*peak* are over the load window.
- **Throughput:** a built-in Rust/tokio load generator against
  `/ssr` (50 connections, 10s, after warm-up).
- **Parallelism** = `OSSIDO_SSR_THREADS` for Ossido, Node `cluster` worker
  count (`WEB_CONCURRENCY`) for Next.js.
- Both are production builds rendering the identical component tree.
