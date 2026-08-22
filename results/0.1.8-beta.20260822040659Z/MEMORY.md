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
| Date | 2026-08-22T15:04:33.083Z |
| Host | Darwin 25.5.0 · arm64 |
| CPU | Apple M4 Max |
| Logical cores | 16 |
| Memory | 48.0 GB |
| Load | 50 connections, 10s (+3s warm-up), route `/ssr` |

## Results

| Framework | Parallelism | Idle RSS (MB) | Mean RSS (MB) | Peak RSS (MB) | req/s | **req/s per MB** |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Ossido | 1 | 36 | 375 | 544 | 2,058 | **5.5** |
| Ossido | 2 | 41 | 616 | 885 | 3,246 | **5.3** |
| Ossido | 4 | 50 | 1,084 | 1,528 | 5,411 | **5.0** |
| Ossido | 8 | 70 | 1,906 | 2,648 | 8,421 | **4.4** |
| Ossido | 16 | 108 | 3,361 | 4,544 | 7,932 | **2.4** |
| Next.js | 1 | 117 | 563 | 620 | 578 | **1.0** |
| Next.js | 2 | 295 | 1,085 | 1,208 | 1,148 | **1.1** |
| Next.js | 4 | 511 | 1,827 | 1,921 | 2,229 | **1.2** |
| Next.js | 8 | 942 | 3,469 | 3,648 | 3,905 | **1.1** |
| Next.js | 16 | 1,813 | 6,549 | 7,257 | 4,225 | **0.6** |

## Headline

At full parallelism (16 threads / workers), serving `/ssr`:

- **Ossido:** 7,932 req/s using 3,361 MB → **2.4 req/s per MB**
- **Next.js:** 4,225 req/s using 6,549 MB → **0.6 req/s per MB**

➡️ Ossido is **3.7× more memory-efficient** here, and uses **1.9× less RAM** (3,361 MB vs 6,549 MB) while serving 1.88× the throughput.

**Baseline footprint:** the gap is starkest at idle. Scaling Ossido from 1 to 16 threads adds only **72 MB** (shared process, 108 MB total); scaling Next.js to 16 workers costs **1,813 MB** — **17× more** just to sit idle, because every worker is a full Node.js + Next.js heap.

**Iso-memory:** within Ossido's 3,361 MB footprint, Next.js can only run 4 worker(s) (1,827 MB, 2,229 req/s) — Ossido serves 3.6× the requests in the same RAM.

## Charts

*Bars are the parallelism sweep: `Oss xn` = Ossido with n threads, `Next xn` =
Next.js with n workers.*

### Throughput vs parallelism (req/s — higher is better)

```mermaid
xychart-beta
    title "Throughput — req/s"
    x-axis ["Oss x1", "Oss x2", "Oss x4", "Oss x8", "Oss x16", "Next x1", "Next x2", "Next x4", "Next x8", "Next x16"]
    y-axis "req/s"
    bar [2058.2, 3245.6, 5410.8, 8420.8, 7931.6, 578.3, 1148.41, 2229.4, 3905.4, 4225.4]
```

### Memory vs parallelism (mean RSS, MB — lower is better)

```mermaid
xychart-beta
    title "Memory — mean RSS (MB)"
    x-axis ["Oss x1", "Oss x2", "Oss x4", "Oss x8", "Oss x16", "Next x1", "Next x2", "Next x4", "Next x8", "Next x16"]
    y-axis "MB"
    bar [375.19, 616.06, 1083.62, 1906.42, 3360.59, 562.59, 1084.8, 1827.44, 3468.85, 6549.34]
```

### Efficiency vs parallelism (req/s per MB — higher is better)

This is the direct test of the hypothesis.

```mermaid
xychart-beta
    title "Efficiency — req/s per MB"
    x-axis ["Oss x1", "Oss x2", "Oss x4", "Oss x8", "Oss x16", "Next x1", "Next x2", "Next x4", "Next x8", "Next x16"]
    y-axis "req/s per MB"
    bar [5.49, 5.27, 4.99, 4.42, 2.36, 1.03, 1.06, 1.22, 1.13, 0.65]
```

---

### How this is measured

- **Memory:** RSS of the entire server process group (Ossido: one process
  with N render threads; Next.js: the cluster primary + N workers), summed
  from `ps` and sampled every 250ms during the load. *Idle* is captured just
  before load; *mean*/*peak* are over the load window.
- **Throughput:** [autocannon](https://github.com/mcollina/autocannon) against
  `/ssr` (50 connections, 10s, after warm-up).
- **Parallelism** = `OSSIDO_SSR_THREADS` for Ossido, Node `cluster` worker
  count (`WEB_CONCURRENCY`) for Next.js.
- Both are production builds rendering the identical component tree.
