//! The common `Benchmark` abstraction. A `Benchmark` is generic over its own
//! `Output` type (each produces its own record set and writes its own reports),
//! yet the object-safe `DynBenchmark` driver lets a single `run` command drive a
//! heterogeneous `Vec<Box<dyn DynBenchmark>>` uniformly.

use anyhow::Result;
use async_trait::async_trait;

use crate::config::Config;
use crate::ui::Progress;

/// Shared, read-only context handed to every benchmark.
pub struct BenchCtx {
    pub cores: usize,
    pub config: Config,
    /// False when any `BENCH_*` override is set — the run is print-only.
    pub outputs_enabled: bool,
    /// Shared keep-alive client for health polls and the stream probe.
    pub http: reqwest::Client,
}

impl BenchCtx {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(BenchCtx {
            cores: crate::config::cpu_count(),
            config: Config::from_env(),
            outputs_enabled: crate::config::outputs_enabled(),
            http,
        })
    }
}

/// A benchmark that produces a typed output and knows how to persist it.
#[async_trait]
pub trait Benchmark: Send + Sync {
    /// Structured result of one full benchmark pass (own type per benchmark).
    type Output: Send;

    fn name(&self) -> &'static str;

    /// Execute the whole benchmark. `progress` lets it spin up one child
    /// spinner per scenario/test.
    async fn run_benchmark(&self, ctx: &BenchCtx, progress: &Progress) -> Result<Self::Output>;

    /// Post-run side effects: write reports + update the README table, gated on
    /// `ctx.outputs_enabled`. Receives the typed output so each benchmark
    /// serializes its own schema.
    async fn on_completion(&self, ctx: &BenchCtx, output: Self::Output) -> Result<()>;
}

/// Object-safe driver: runs a benchmark and its completion side effects,
/// consuming the associated `Output` internally so it never crosses the `dyn`
/// boundary. A blanket impl covers every `Benchmark`.
#[async_trait]
pub trait DynBenchmark: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run_and_finish(&self, ctx: &BenchCtx, progress: &Progress) -> Result<()>;
}

#[async_trait]
impl<B: Benchmark> DynBenchmark for B {
    fn name(&self) -> &'static str {
        Benchmark::name(self)
    }

    async fn run_and_finish(&self, ctx: &BenchCtx, progress: &Progress) -> Result<()> {
        let output = self.run_benchmark(ctx, progress).await?;
        self.on_completion(ctx, output).await
    }
}
