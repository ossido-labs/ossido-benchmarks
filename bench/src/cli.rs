//! clap subcommand surface. Mirrors the old npm scripts (bench, bench:build,
//! bench:memory, bench:index, bench:version, bench:all).

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bench", about = "Ossido vs Next.js benchmark harness", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run all benchmarks (throughput, then memory efficiency).
    Run {
        /// Build both apps first.
        #[arg(long)]
        build: bool,
    },
    /// Throughput + latency + streaming benchmark.
    Throughput {
        /// Build both apps first.
        #[arg(long)]
        build: bool,
    },
    /// Memory-efficiency sweep across parallelism levels.
    Memory,
    /// Pin the Ossido example to a version, rebuild, and run both benchmarks.
    Version {
        /// The Ossido framework version, e.g. 0.1.7.
        version: String,
    },
    /// Build both example apps.
    Build,
    /// Rewrite the README results table.
    Index,
}
