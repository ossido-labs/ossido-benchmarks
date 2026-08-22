import autocannon from 'autocannon';
import { CONNECTIONS, DURATION, PIPELINING, WARMUP, TTFB_SAMPLES } from './config.ts';

// Per-request timeout (seconds). Generous so a slow, saturated CPU-bound render
// completes and counts toward throughput instead of being recorded as an error.
const REQUEST_TIMEOUT = 30;

export interface LoadResult {
  rps: number; // requests / second (mean)
  latencyMean: number; // ms
  latencyP50: number; // ms
  latencyP99: number; // ms
  throughputMbps: number; // MB / second
  errors: number; // non-2xx + socket errors + timeouts
  connections: number;
}

function runAutocannon(
  url: string,
  duration: number,
  connections: number,
): Promise<autocannon.Result> {
  return new Promise((resolve, reject) => {
    autocannon(
      { url, connections, pipelining: PIPELINING, duration, timeout: REQUEST_TIMEOUT },
      (err, result) => (err ? reject(err) : resolve(result)),
    );
  });
}

export async function loadTest(url: string, connections = CONNECTIONS): Promise<LoadResult> {
  // Warm-up pass (discarded) so V8 tiering / JIT and connection pools settle.
  if (WARMUP > 0) await runAutocannon(url, WARMUP, connections);
  const r = await runAutocannon(url, DURATION, connections);
  const errors =
    (r.non2xx ?? 0) + (r.errors ?? 0) + (r.timeouts ?? 0);
  return {
    rps: r.requests.mean,
    latencyMean: r.latency.mean,
    latencyP50: r.latency.p50,
    latencyP99: r.latency.p99,
    throughputMbps: r.throughput.mean / (1024 * 1024),
    errors,
    connections,
  };
}

export interface StreamResult {
  ttfbMs: number; // median time-to-first-byte
  totalMs: number; // median time-to-full-response
  bytes: number;
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

/**
 * Sequentially probe a streaming route, timing the first byte separately from
 * the full response. This is what shows the streaming benefit: TTFB (the shell)
 * lands well before the total (the streamed heavy section).
 */
export async function streamProbe(url: string): Promise<StreamResult> {
  const ttfb: number[] = [];
  const total: number[] = [];
  let bytes = 0;
  const samples = TTFB_SAMPLES + 3; // first 3 are warm-up
  for (let i = 0; i < samples; i++) {
    const start = performance.now();
    const res = await fetch(url);
    const reader = res.body!.getReader();
    let first = -1;
    let size = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (first < 0) first = performance.now();
      size += value.byteLength;
    }
    const end = performance.now();
    if (i >= 3) {
      ttfb.push((first < 0 ? end : first) - start);
      total.push(end - start);
      bytes = size;
    }
  }
  return { ttfbMs: median(ttfb), totalMs: median(total), bytes };
}
