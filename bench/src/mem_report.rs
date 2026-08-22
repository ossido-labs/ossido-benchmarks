//! Memory-efficiency report: MEMORY.md + memory.json. Ported from `memory.ts`,
//! preserving the hypothesis text, headline computations, sweep charts, and the
//! `ossido-benchmark/memory@1` JSON schema.

use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;

use crate::config::{FrameworkKey, Versions, MEM_PATH};
use crate::load::LoadResult;
use crate::report::fmt;

pub struct MemRecord {
    pub framework: FrameworkKey,
    pub parallelism: usize,
    pub idle_rss_mb: f64,
    pub mean_rss_mb: f64,
    pub peak_rss_mb: f64,
    pub load: LoadResult,
}

pub struct MemoryReport {
    pub generated_at: String,
    pub cores: usize,
    pub host: String,
    pub cpu_model: String,
    pub total_mem_gb: f64,
    pub connections: usize,
    pub duration_sec: u64,
    pub warmup_sec: u64,
    pub versions: Versions,
    pub records: Vec<MemRecord>,
}

/// Parallelism levels to sweep: powers of two up to the core count, inclusive.
pub fn levels(cores: usize) -> Vec<usize> {
    let mut set: Vec<usize> = vec![1];
    let mut n = 2;
    while n <= cores {
        set.push(n);
        n *= 2;
    }
    set.push(cores);
    set.sort_unstable();
    set.dedup();
    set
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn eff_of(r: &MemRecord) -> f64 {
    r.load.rps / r.mean_rss_mb
}

/// One bar chart over both frameworks' parallelism sweep (Ossido then Next.js).
fn mermaid_sweep_bar(title: &str, y_label: &str, xs: &[usize], ossido: &[f64], next: &[f64]) -> String {
    // NB: ASCII 'x' (not '×') — the mermaid grammar rejects U+00D7 in labels.
    let mut labels: Vec<String> = xs.iter().map(|x| format!("\"Oss x{x}\"")).collect();
    labels.extend(xs.iter().map(|x| format!("\"Next x{x}\"")));
    let values: Vec<String> = ossido
        .iter()
        .chain(next.iter())
        .map(|v| round2(*v).to_string())
        .collect();
    [
        "```mermaid",
        "xychart-beta",
        &format!("    title \"{title}\""),
        &format!("    x-axis [{}]", labels.join(", ")),
        &format!("    y-axis \"{y_label}\""),
        &format!("    bar [{}]", values.join(", ")),
        "```",
        "",
    ]
    .join("\n")
}

pub fn memory_markdown(report: &MemoryReport) -> String {
    let cores = report.cores;
    let xs = levels(cores);
    let get: HashMap<(FrameworkKey, usize), &MemRecord> =
        report.records.iter().map(|r| ((r.framework, r.parallelism), r)).collect();
    let get = |fw: FrameworkKey, n: usize| get.get(&(fw, n)).copied();

    let mut lines: Vec<String> = Vec::new();
    let mut p = |s: String| lines.push(s);

    p("# Ossido vs Next.js — memory efficiency".into());
    p(String::new());
    p("> Hypothesis: **Ossido serves far more requests per unit of memory.**".into());
    p(">".into());
    p("> Ossido scales SSR across cores with V8 render threads inside a *single*".into());
    p("> Rust process (one shared heap); Next.js scales by forking *N* full Node.js".into());
    p("> processes (one heap each). This sweep runs the same `/ssr` load at each".into());
    p("> parallelism level while sampling the resident memory (RSS) of the whole".into());
    p("> server process group, and reports **req/s per MB**.".into());
    p(String::new());
    p("## Environment".into());
    p(String::new());
    p("| | |".into());
    p("| --- | --- |".into());
    p(format!("| Date | {} |", report.generated_at));
    p(format!("| Host | {} |", report.host));
    p(format!("| CPU | {} |", report.cpu_model));
    p(format!("| Logical cores | {cores} |"));
    p(format!("| Memory | {:.1} GB |", report.total_mem_gb));
    p(format!(
        "| Load | {} connections, {}s (+{}s warm-up), route `{}` |",
        report.connections, report.duration_sec, report.warmup_sec, MEM_PATH
    ));
    p(String::new());

    // ---- Main table ----
    p("## Results".into());
    p(String::new());
    p("| Framework | Parallelism | Idle RSS (MB) | Mean RSS (MB) | Peak RSS (MB) | req/s | **req/s per MB** |".into());
    p("| --- | ---: | ---: | ---: | ---: | ---: | ---: |".into());
    for fw in [FrameworkKey::Ossido, FrameworkKey::Next] {
        for &n in &xs {
            if let Some(r) = get(fw, n) {
                let name = if fw == FrameworkKey::Ossido { "Ossido" } else { "Next.js" };
                p(format!(
                    "| {name} | {n} | {} | {} | {} | {} | **{}** |",
                    fmt(r.idle_rss_mb, 0),
                    fmt(r.mean_rss_mb, 0),
                    fmt(r.peak_rss_mb, 0),
                    fmt(r.load.rps, 0),
                    fmt(eff_of(r), 1)
                ));
            }
        }
    }
    p(String::new());

    // ---- Headline ----
    if let (Some(o_max), Some(n_max)) = (get(FrameworkKey::Ossido, cores), get(FrameworkKey::Next, cores)) {
        let o_eff = eff_of(o_max);
        let n_eff = eff_of(n_max);
        p("## Headline".into());
        p(String::new());
        p(format!("At full parallelism ({cores} threads / workers), serving `/ssr`:"));
        p(String::new());
        p(format!("- **Ossido:** {} req/s using {} MB → **{} req/s per MB**", fmt(o_max.load.rps, 0), fmt(o_max.mean_rss_mb, 0), fmt(o_eff, 1)));
        p(format!("- **Next.js:** {} req/s using {} MB → **{} req/s per MB**", fmt(n_max.load.rps, 0), fmt(n_max.mean_rss_mb, 0), fmt(n_eff, 1)));
        p(String::new());
        p(format!(
            "➡️ Ossido is **{:.1}× more memory-efficient** here, and uses **{:.1}× less RAM** ({} MB vs {} MB) while serving {:.2}× the throughput.",
            o_eff / n_eff,
            n_max.mean_rss_mb / o_max.mean_rss_mb,
            fmt(o_max.mean_rss_mb, 0),
            fmt(n_max.mean_rss_mb, 0),
            o_max.load.rps / n_max.load.rps
        ));
        p(String::new());
        let o1_idle = get(FrameworkKey::Ossido, 1).map(|r| r.idle_rss_mb).unwrap_or(0.0);
        p(format!(
            "**Baseline footprint:** the gap is starkest at idle. Scaling Ossido from 1 to {cores} threads adds only **{} MB** (shared process, {} MB total); scaling Next.js to {cores} workers costs **{} MB** — **{:.0}× more** just to sit idle, because every worker is a full Node.js + Next.js heap.",
            fmt(o_max.idle_rss_mb - o1_idle, 0),
            fmt(o_max.idle_rss_mb, 0),
            fmt(n_max.idle_rss_mb, 0),
            n_max.idle_rss_mb / o_max.idle_rss_mb
        ));
        p(String::new());

        // Iso-memory framing.
        let budget = o_max.mean_rss_mb;
        let fits: Vec<usize> = xs.iter().copied().filter(|&n| get(FrameworkKey::Next, n).map(|r| r.mean_rss_mb).unwrap_or(f64::INFINITY) <= budget).collect();
        let best_next_fit = fits.iter().copied().max().and_then(|n| get(FrameworkKey::Next, n));
        let tail = match best_next_fit {
            Some(b) => format!(
                "run {} worker(s) ({} MB, {} req/s) — Ossido serves {:.1}× the requests in the same RAM.",
                b.parallelism,
                fmt(b.mean_rss_mb, 0),
                fmt(b.load.rps, 0),
                o_max.load.rps / b.load.rps
            ),
            None => "not even fit a single worker at these levels.".into(),
        };
        p(format!("**Iso-memory:** within Ossido's {} MB footprint, Next.js can only {tail}", fmt(budget, 0)));
        p(String::new());
    }

    // ---- Charts ----
    p("## Charts".into());
    p(String::new());
    p("*Bars are the parallelism sweep: `Oss xn` = Ossido with n threads, `Next xn` =".into());
    p("Next.js with n workers.*".into());
    p(String::new());

    p("### Throughput vs parallelism (req/s — higher is better)".into());
    p(String::new());
    p(mermaid_sweep_bar(
        "Throughput — req/s",
        "req/s",
        &xs,
        &xs.iter().map(|&n| get(FrameworkKey::Ossido, n).map(|r| r.load.rps).unwrap_or(0.0)).collect::<Vec<_>>(),
        &xs.iter().map(|&n| get(FrameworkKey::Next, n).map(|r| r.load.rps).unwrap_or(0.0)).collect::<Vec<_>>(),
    ));

    p("### Memory vs parallelism (mean RSS, MB — lower is better)".into());
    p(String::new());
    p(mermaid_sweep_bar(
        "Memory — mean RSS (MB)",
        "MB",
        &xs,
        &xs.iter().map(|&n| get(FrameworkKey::Ossido, n).map(|r| r.mean_rss_mb).unwrap_or(0.0)).collect::<Vec<_>>(),
        &xs.iter().map(|&n| get(FrameworkKey::Next, n).map(|r| r.mean_rss_mb).unwrap_or(0.0)).collect::<Vec<_>>(),
    ));

    p("### Efficiency vs parallelism (req/s per MB — higher is better)".into());
    p(String::new());
    p("This is the direct test of the hypothesis.".into());
    p(String::new());
    p(mermaid_sweep_bar(
        "Efficiency — req/s per MB",
        "req/s per MB",
        &xs,
        &xs.iter().map(|&n| get(FrameworkKey::Ossido, n).map(eff_of).unwrap_or(0.0)).collect::<Vec<_>>(),
        &xs.iter().map(|&n| get(FrameworkKey::Next, n).map(eff_of).unwrap_or(0.0)).collect::<Vec<_>>(),
    ));

    p("---".into());
    p(String::new());
    p("### How this is measured".into());
    p(String::new());
    p("- **Memory:** RSS of the entire server process group (Ossido: one process".into());
    p("  with N render threads; Next.js: the cluster primary + N workers), summed".into());
    p("  and sampled every 250ms during the load. *Idle* is captured just".into());
    p("  before load; *mean*/*peak* are over the load window.".into());
    p("- **Throughput:** a built-in Rust/tokio load generator against".into());
    p(format!("  `{}` ({} connections, {}s, after warm-up).", MEM_PATH, report.connections, report.duration_sec));
    p("- **Parallelism** = `OSSIDO_SSR_THREADS` for Ossido, Node `cluster` worker".into());
    p("  count (`WEB_CONCURRENCY`) for Next.js.".into());
    p("- Both are production builds rendering the identical component tree.".into());
    p(String::new());

    lines.join("\n")
}

// ── JSON (schema ossido-benchmark/memory@1) ─────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Environment {
    host: String,
    cpu: String,
    cores: usize,
    total_mem_gb: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadMeta {
    connections: usize,
    duration_sec: u64,
    warmup_sec: u64,
    route: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemResultJson {
    framework: &'static str,
    parallelism: usize,
    idle_rss_mb: f64,
    mean_rss_mb: f64,
    peak_rss_mb: f64,
    rps: f64,
    req_per_mb: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryJson {
    schema: &'static str,
    generated_at: String,
    environment: Environment,
    versions: Versions,
    load: LoadMeta,
    levels: Vec<usize>,
    results: Vec<MemResultJson>,
}

pub fn memory_json(report: &MemoryReport) -> Result<String> {
    let json = MemoryJson {
        schema: "ossido-benchmark/memory@1",
        generated_at: report.generated_at.clone(),
        environment: Environment {
            host: report.host.clone(),
            cpu: report.cpu_model.clone(),
            cores: report.cores,
            total_mem_gb: report.total_mem_gb,
        },
        versions: report.versions.clone(),
        load: LoadMeta {
            connections: report.connections,
            duration_sec: report.duration_sec,
            warmup_sec: report.warmup_sec,
            route: MEM_PATH,
        },
        levels: levels(report.cores),
        results: report
            .records
            .iter()
            .map(|r| MemResultJson {
                framework: r.framework.as_str(),
                parallelism: r.parallelism,
                idle_rss_mb: round1(r.idle_rss_mb),
                mean_rss_mb: round1(r.mean_rss_mb),
                peak_rss_mb: round1(r.peak_rss_mb),
                rps: r.load.rps,
                req_per_mb: round2(r.load.rps / r.mean_rss_mb),
            })
            .collect(),
    };
    Ok(serde_json::to_string_pretty(&json)? + "\n")
}
