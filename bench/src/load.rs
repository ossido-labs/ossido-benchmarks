//! HTTP load generation (replaces autocannon) and the sequential streaming
//! probe. Model: N persistent keep-alive workers, each looping requests for a
//! fixed wall-clock duration, aggregated into a merged HDR histogram.

use std::time::{Duration, Instant};

use anyhow::Result;
use futures_util::StreamExt;
use hdrhistogram::Histogram;

use crate::config::Config;

/// Generous per-request timeout so a slow, saturated CPU-bound render still
/// completes and counts toward throughput instead of being recorded as an error.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Histogram covers 1µs..60s at 3 significant figures.
const HIST_MAX_US: u64 = 60_000_000;

#[derive(Clone)]
pub struct LoadResult {
    pub rps: f64,
    pub latency_mean: f64, // ms
    pub latency_p50: f64,  // ms
    pub latency_p99: f64,  // ms
    pub throughput_mbps: f64,
    pub errors: u64,
    pub connections: usize,
}

struct WorkerStats {
    hist: Histogram<u64>,
    count: u64,
    bytes: u64,
    errors: u64,
}

fn new_hist() -> Histogram<u64> {
    Histogram::<u64>::new_with_bounds(1, HIST_MAX_US, 3).expect("valid histogram bounds")
}

/// Brief pause after a failed request to avoid a hot reconnect loop.
async fn backoff() {
    tokio::time::sleep(Duration::from_millis(2)).await;
}

async fn worker(url: String, deadline: Instant) -> WorkerStats {
    // One client per worker with a single idle slot = one guaranteed live
    // keep-alive socket, the closest analog to an autocannon connection.
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(1)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .expect("client builds");

    let mut hist = new_hist();
    let (mut count, mut bytes, mut errors) = (0u64, 0u64, 0u64);

    while Instant::now() < deadline {
        let start = Instant::now();
        match client.get(&url).send().await {
            Ok(resp) => {
                let ok = resp.status().is_success();
                match resp.bytes().await {
                    Ok(body) => {
                        let us = (start.elapsed().as_micros() as u64).clamp(1, HIST_MAX_US);
                        let _ = hist.record(us);
                        count += 1;
                        bytes += body.len() as u64;
                        if !ok {
                            errors += 1; // non-2xx
                        }
                    }
                    Err(_) => {
                        errors += 1; // read/timeout error
                        backoff().await;
                    }
                }
            }
            Err(_) => {
                errors += 1; // connect/send/timeout error
                // A connection error (e.g. the server reset us under overload)
                // means a new socket next time; a short backoff curbs a reconnect
                // storm that would otherwise exhaust localhost ephemeral ports.
                backoff().await;
            }
        }
    }

    WorkerStats { hist, count, bytes, errors }
}

/// Run one measured pass; returns merged stats and the actual elapsed time.
async fn run_pass(url: &str, connections: usize, duration: u64) -> (Histogram<u64>, u64, u64, u64, Duration) {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(duration);

    let mut handles = Vec::with_capacity(connections);
    for _ in 0..connections {
        handles.push(tokio::spawn(worker(url.to_string(), deadline)));
    }

    let mut hist = new_hist();
    let (mut count, mut bytes, mut errors) = (0u64, 0u64, 0u64);
    for h in handles {
        if let Ok(s) = h.await {
            let _ = hist.add(&s.hist);
            count += s.count;
            bytes += s.bytes;
            errors += s.errors;
        }
    }
    (hist, count, bytes, errors, start.elapsed())
}

/// Warm-up pass (discarded) then a measured pass, mirroring `load.ts`.
pub async fn load_test(url: &str, connections: usize, cfg: &Config) -> LoadResult {
    if cfg.warmup > 0 {
        let _ = run_pass(url, connections, cfg.warmup).await;
    }
    let (hist, count, bytes, errors, elapsed) = run_pass(url, connections, cfg.duration).await;
    let secs = elapsed.as_secs_f64().max(f64::MIN_POSITIVE);
    LoadResult {
        rps: count as f64 / secs,
        latency_mean: hist.mean() / 1000.0,
        latency_p50: hist.value_at_quantile(0.50) as f64 / 1000.0,
        latency_p99: hist.value_at_quantile(0.99) as f64 / 1000.0,
        throughput_mbps: bytes as f64 / secs / (1024.0 * 1024.0),
        errors,
        connections,
    }
}

#[derive(Clone)]
pub struct StreamResult {
    pub ttfb_ms: f64,
    pub total_ms: f64,
    pub bytes: u64,
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    }
}

/// A single streaming probe: TTFB, total, and body size.
async fn probe_once(client: &reqwest::Client, url: &str) -> Result<(f64, f64, u64)> {
    let start = Instant::now();
    let resp = client.get(url).send().await?;
    let mut stream = resp.bytes_stream();
    let mut first: Option<Instant> = None;
    let mut size = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if first.is_none() {
            first = Some(Instant::now());
        }
        size += chunk.len() as u64;
    }
    let end = Instant::now();
    let first_at = first.unwrap_or(end);
    Ok(((first_at - start).as_secs_f64() * 1000.0, (end - start).as_secs_f64() * 1000.0, size))
}

/// Sequentially probe a streaming route, timing the first streamed chunk (shell)
/// separately from the full response. First 3 iterations are warm-up. Transient
/// connect errors (e.g. localhost ephemeral-port pressure right after a heavy
/// load pass) are retried so the probe survives a busy machine.
pub async fn stream_probe(client: &reqwest::Client, url: &str, samples: usize) -> Result<StreamResult> {
    let total_iters = samples + 3;
    let mut ttfb = Vec::new();
    let mut total = Vec::new();
    let mut bytes = 0u64;

    for i in 0..total_iters {
        let mut attempt = 0;
        let (t, tot, size) = loop {
            match probe_once(client, url).await {
                Ok(v) => break v,
                Err(e) => {
                    attempt += 1;
                    if attempt >= 60 {
                        return Err(e);
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            }
        };
        if i >= 3 {
            ttfb.push(t);
            total.push(tot);
            bytes = size;
        }
    }

    Ok(StreamResult { ttfb_ms: median(&ttfb), total_ms: median(&total), bytes })
}
