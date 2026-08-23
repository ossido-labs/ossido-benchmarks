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
| Date | 2026-08-23T02:09:33.522858Z |
| Host | Darwin 25.5.0 · aarch64 |
| CPU | Apple M4 Max |
| Logical cores | 16 |
| Memory | 48.0 GB |
| Load | 50 connections, 10s (+3s warm-up), route `/ssr` |

## Results

| Framework | Parallelism | Idle RSS (MB) | Mean RSS (MB) | Peak RSS (MB) | req/s | **req/s per MB** |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Ossido | 1 | 34 | 376 | 543 | 2,052 | **5.5** |
| Ossido | 2 | 40 | 640 | 923 | 3,449 | **5.4** |
| Ossido | 4 | 49 | 1,107 | 1,573 | 5,618 | **5.1** |
| Ossido | 8 | 69 | 2,487 | 3,755 | 9,074 | **3.6** |
| Ossido | 16 | 109 | 3,239 | 4,455 | 9,120 | **2.8** |
| Next.js | 1 | 117 | 489 | 515 | 591 | **1.2** |
| Next.js | 2 | 294 | 947 | 983 | 1,154 | **1.2** |
| Next.js | 4 | 511 | 1,771 | 1,874 | 2,154 | **1.2** |
| Next.js | 8 | 944 | 3,505 | 3,701 | 3,727 | **1.1** |
| Next.js | 16 | 1,810 | 6,270 | 6,960 | 4,035 | **0.6** |

## Headline

At full parallelism (16 threads / workers), serving `/ssr`:

- **Ossido:** 9,120 req/s using 3,239 MB → **2.8 req/s per MB**
- **Next.js:** 4,035 req/s using 6,270 MB → **0.6 req/s per MB**

➡️ Ossido is **4.4× more memory-efficient** here, and uses **1.9× less RAM** (3,239 MB vs 6,270 MB) while serving 2.26× the throughput.

**Baseline footprint:** the gap is starkest at idle. Scaling Ossido from 1 to 16 threads adds only **75 MB** (shared process, 109 MB total); scaling Next.js to 16 workers costs **1,810 MB** — **17× more** just to sit idle, because every worker is a full Node.js + Next.js heap.

**Iso-memory:** within Ossido's 3,239 MB footprint, Next.js can only run 4 worker(s) (1,771 MB, 2,154 req/s) — Ossido serves 4.2× the requests in the same RAM.

## Charts

*Bars are the parallelism sweep: `Oss xn` = Ossido with n threads, `Next xn` =
Next.js with n workers.*

### Throughput vs parallelism (req/s — higher is better)

```mermaid
xychart-beta
    title "Throughput — req/s"
    x-axis ["Oss x1", "Oss x2", "Oss x4", "Oss x8", "Oss x16", "Next x1", "Next x2", "Next x4", "Next x8", "Next x16"]
    y-axis "req/s"
    bar [2051.73, 3449.37, 5617.92, 9074.32, 9119.85, 590.63, 1154.35, 2153.98, 3726.57, 4034.66]
```

### Memory vs parallelism (mean RSS, MB — lower is better)

```mermaid
xychart-beta
    title "Memory — mean RSS (MB)"
    x-axis ["Oss x1", "Oss x2", "Oss x4", "Oss x8", "Oss x16", "Next x1", "Next x2", "Next x4", "Next x8", "Next x16"]
    y-axis "MB"
    bar [375.94, 640.1, 1106.98, 2486.86, 3238.75, 489.24, 946.97, 1771.47, 3504.53, 6269.83]
```

### Efficiency vs parallelism (req/s per MB — higher is better)

This is the direct test of the hypothesis.

```mermaid
xychart-beta
    title "Efficiency — req/s per MB"
    x-axis ["Oss x1", "Oss x2", "Oss x4", "Oss x8", "Oss x16", "Next x1", "Next x2", "Next x4", "Next x8", "Next x16"]
    y-axis "req/s per MB"
    bar [5.46, 5.39, 5.07, 3.65, 2.82, 1.21, 1.22, 1.22, 1.06, 0.64]
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
