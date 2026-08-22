//! Memory-efficiency sweep (port of `memory.ts`). For each framework × parallelism
//! level: start server, capture idle RSS, sample RSS every 250ms during a `/ssr`
//! load, and report mean/peak + req/s per MB.

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use sysinfo::System;

use crate::bench_trait::{BenchCtx, Benchmark};
use crate::build_cmd::ensure_built;
use crate::config::{frameworks, overridden_env_vars, read_versions, results_dir, MEM_PATH};
use crate::index::update_readme_table;
use crate::load::load_test;
use crate::mem_report::{levels, memory_json, memory_markdown, MemRecord, MemoryReport};
use crate::report::fmt;
use crate::server::start_server;
use crate::sys::{env_info, group_rss_mb, now_iso};
use crate::ui::Progress;

pub struct Memory;

#[async_trait]
impl Benchmark for Memory {
    type Output = MemoryReport;

    fn name(&self) -> &'static str {
        "Memory efficiency"
    }

    async fn run_benchmark(&self, ctx: &BenchCtx, progress: &Progress) -> Result<MemoryReport> {
        ensure_built()?;
        let xs = levels(ctx.cores);
        let mut records: Vec<MemRecord> = Vec::new();

        for framework in frameworks() {
            for &n in &xs {
                let h = progress.scenario(&format!("{} ×{n}", framework.label));
                let server = match start_server(&ctx.http, framework, n).await {
                    Ok(s) => s,
                    Err(e) => {
                        h.finish_err(&e.to_string());
                        return Err(e);
                    }
                };
                let pgid = server.pgid;

                let measured: Result<MemRecord> = async {
                    // Let workers finish warming/prepping, then capture idle memory.
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    let mut sys = System::new();
                    let idle = group_rss_mb(&mut sys, pgid);

                    // Sample RSS while the load test runs.
                    let samples = Arc::new(Mutex::new(Vec::<f64>::new()));
                    let stop = Arc::new(AtomicBool::new(false));
                    let sampler = {
                        let samples = Arc::clone(&samples);
                        let stop = Arc::clone(&stop);
                        tokio::spawn(async move {
                            let mut sys = System::new();
                            while !stop.load(Ordering::Relaxed) {
                                tokio::time::sleep(Duration::from_millis(250)).await;
                                let rss = group_rss_mb(&mut sys, pgid);
                                samples.lock().unwrap().push(rss);
                            }
                        })
                    };

                    let url = format!("http://127.0.0.1:{}{}", framework.port, MEM_PATH);
                    let load = load_test(&url, ctx.config.connections, &ctx.config).await;

                    stop.store(true, Ordering::Relaxed);
                    let _ = sampler.await;

                    let samples = samples.lock().unwrap();
                    let (peak, mean) = if samples.is_empty() {
                        (idle, idle)
                    } else {
                        let peak = samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
                        (peak, mean)
                    };

                    Ok(MemRecord {
                        framework: framework.key,
                        parallelism: n,
                        idle_rss_mb: idle,
                        mean_rss_mb: mean,
                        peak_rss_mb: peak,
                        load,
                    })
                }
                .await;

                let _ = server.stop().await;
                // Let ports/pgid fully release before the next server.
                tokio::time::sleep(Duration::from_millis(500)).await;

                match measured {
                    Ok(rec) => {
                        h.finish_ok(&format!(
                            "{} req/s · {} MB · {} req/s per MB",
                            fmt(rec.load.rps, 0),
                            fmt(rec.mean_rss_mb, 0),
                            fmt(rec.load.rps / rec.mean_rss_mb, 1)
                        ));
                        records.push(rec);
                    }
                    Err(e) => {
                        h.finish_err(&e.to_string());
                        return Err(e);
                    }
                }
            }
        }

        let env = env_info();
        Ok(MemoryReport {
            generated_at: now_iso(),
            cores: ctx.cores,
            host: env.host,
            cpu_model: env.cpu_model,
            total_mem_gb: env.total_mem_gb,
            connections: ctx.config.connections,
            duration_sec: ctx.config.duration,
            warmup_sec: ctx.config.warmup,
            versions: read_versions(),
            records,
        })
    }

    async fn on_completion(&self, ctx: &BenchCtx, output: MemoryReport) -> Result<()> {
        if !ctx.outputs_enabled {
            println!(
                "\n⚠ Non-default load ({} set) — skipping output. Run with defaults to update results/<version>/ and the README table.",
                overridden_env_vars().join(", ")
            );
            return Ok(());
        }
        let dir = results_dir();
        fs::create_dir_all(&dir)?;
        let md_path = dir.join("MEMORY.md");
        let json_path = dir.join("memory.json");
        fs::write(&md_path, memory_markdown(&output))?;
        fs::write(&json_path, memory_json(&output)?)?;
        update_readme_table()?;
        println!("\n✔ Reports written to {} and {}", md_path.display(), json_path.display());
        Ok(())
    }
}
