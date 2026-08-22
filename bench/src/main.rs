mod bench_trait;
mod benchmarks;
mod build_cmd;
mod cli;
mod config;
mod index;
mod load;
mod mem_report;
mod report;
mod server;
mod sys;
mod ui;
mod version_cmd;

use anyhow::Result;
use clap::Parser;

use bench_trait::{BenchCtx, DynBenchmark};
use benchmarks::{Memory, Throughput};
use build_cmd::build_all;
use cli::{Cli, Command};
use index::update_readme_table;
use ui::Ui;

#[tokio::main]
async fn main() {
    if let Err(err) = real_main().await {
        eprintln!("\nError: {err:#}");
        std::process::exit(1);
    }
}

async fn real_main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { build } => {
            if build {
                build_all()?;
            }
            run_benchmarks(vec![Box::new(Throughput), Box::new(Memory)]).await?;
        }
        Command::Throughput { build } => {
            if build {
                build_all()?;
            }
            run_benchmarks(vec![Box::new(Throughput)]).await?;
        }
        Command::Memory => run_benchmarks(vec![Box::new(Memory)]).await?,
        Command::Version { version } => version_cmd::run_version(&version).await?,
        Command::Build => build_all()?,
        Command::Index => {
            update_readme_table()?;
            println!("✔ README results table updated.");
        }
    }
    Ok(())
}

/// Drive a heterogeneous set of benchmarks uniformly through the object-safe
/// `DynBenchmark` interface, giving each its own parent + per-scenario spinners.
async fn run_benchmarks(benches: Vec<Box<dyn DynBenchmark>>) -> Result<()> {
    let ctx = BenchCtx::new()?;
    println!(
        "Ossido vs Next.js · {} cores · {} connections · {}s (+{}s warm-up)\n",
        ctx.cores, ctx.config.connections, ctx.config.duration, ctx.config.warmup
    );
    let ui = Ui::new();
    for bench in benches {
        let progress = ui.benchmark(bench.name());
        bench.run_and_finish(&ctx, &progress).await?;
        progress.finish(&format!("{} — done", bench.name()));
    }
    Ok(())
}
