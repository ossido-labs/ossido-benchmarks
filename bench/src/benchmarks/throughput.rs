//! Throughput + latency + streaming benchmark (port of `run.ts`). Loops
//! frameworks × thread modes, running each load scenario then the stream probe.

use std::fs;

use anyhow::Result;
use async_trait::async_trait;

use crate::bench_trait::{BenchCtx, Benchmark};
use crate::build_cmd::ensure_built;
use crate::config::{
    frameworks, load_scenarios, overridden_env_vars, read_versions, results_dir, stream_scenario,
    THREAD_MODES,
};
use crate::index::update_readme_table;
use crate::load::{load_test, stream_probe};
use crate::report::{fmt, results_json, results_markdown, BenchReport, LoadRecord, StreamRecord};
use crate::server::start_server;
use crate::sys::{env_info, now_iso};
use crate::ui::Progress;

pub struct Throughput;

#[async_trait]
impl Benchmark for Throughput {
    type Output = BenchReport;

    fn name(&self) -> &'static str {
        "Throughput"
    }

    async fn run_benchmark(&self, ctx: &BenchCtx, progress: &Progress) -> Result<BenchReport> {
        ensure_built()?;
        let scenarios = load_scenarios();
        let stream = stream_scenario();
        let mut load_records: Vec<LoadRecord> = Vec::new();
        let mut stream_records: Vec<StreamRecord> = Vec::new();

        for framework in frameworks() {
            for mode in THREAD_MODES {
                let threads = mode.threads();
                let label = format!(
                    "{} · {} ({threads} thread{})",
                    framework.label,
                    mode.as_str(),
                    if threads == 1 { "" } else { "s" }
                );
                progress.set_stage(&format!("{label} — starting server…"));
                let server = start_server(&ctx.http, framework, threads).await?;

                // Run all scenarios, but always stop the server afterward.
                let outcome: Result<()> = async {
                    for s in &scenarios {
                        let h = progress.scenario(&format!("{label} · {}", s.title));
                        let conns = s.connections.unwrap_or(ctx.config.connections);
                        let url = format!("http://127.0.0.1:{}{}", framework.port, s.path);
                        let r = load_test(&url, conns, &ctx.config).await;
                        let errs = if r.errors > 0 { format!(" · {} errors", r.errors) } else { String::new() };
                        h.finish_ok(&format!("{} req/s · p99 {:.1}ms{errs}", fmt(r.rps, 0), r.latency_p99));
                        load_records.push(LoadRecord {
                            framework: framework.key,
                            mode,
                            threads,
                            scenario: s.key.to_string(),
                            result: r,
                        });
                    }

                    let h = progress.scenario(&format!("{label} · {}", stream.title));
                    let url = format!("http://127.0.0.1:{}{}", framework.port, stream.path);
                    let sr = stream_probe(&ctx.http, &url, ctx.config.ttfb_samples).await?;
                    h.finish_ok(&format!("TTFB {:.1}ms · total {:.1}ms", sr.ttfb_ms, sr.total_ms));
                    stream_records.push(StreamRecord { framework: framework.key, mode, threads, result: sr });
                    Ok(())
                }
                .await;

                let _ = server.stop().await;
                outcome?;
                // Let ports/pgid release (TIME_WAIT drain) before the next server.
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }
        }

        let env = env_info();
        Ok(BenchReport {
            generated_at: now_iso(),
            cores: ctx.cores,
            host: env.host,
            cpu_model: env.cpu_model,
            total_mem_gb: env.total_mem_gb,
            connections: ctx.config.connections,
            duration_sec: ctx.config.duration,
            warmup_sec: ctx.config.warmup,
            versions: read_versions(),
            load_records,
            stream_records,
        })
    }

    async fn on_completion(&self, ctx: &BenchCtx, output: BenchReport) -> Result<()> {
        if !ctx.outputs_enabled {
            println!(
                "\n⚠ Non-default load ({} set) — skipping output. Run with defaults to update results/<version>/ and the README table.",
                overridden_env_vars().join(", ")
            );
            return Ok(());
        }
        let dir = results_dir();
        fs::create_dir_all(&dir)?;
        let md_path = dir.join("RESULTS.md");
        let json_path = dir.join("results.json");
        fs::write(&md_path, results_markdown(&output))?;
        fs::write(&json_path, results_json(&output)?)?;
        update_readme_table()?;
        println!("\n✔ Reports written to {} and {}", md_path.display(), json_path.display());
        Ok(())
    }
}
