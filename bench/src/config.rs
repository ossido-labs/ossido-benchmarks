//! Ported from the TypeScript `bench/config.ts`: paths, load parameters,
//! framework/scenario definitions, version detection, and output gating.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Repo root — resolved at compile time from this crate's manifest dir
/// (`.../bench`) so the binary always knows the workspace layout regardless of
/// the current working directory.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("bench crate must have a parent (the repo root)")
        .to_path_buf()
}

pub fn ossido_dir() -> PathBuf {
    root().join("examples/ossido")
}

pub fn next_dir() -> PathBuf {
    root().join("examples/next")
}

/// The Ossido example is a workspace member, so its release binary lands in the
/// workspace-root target dir.
pub fn ossido_binary() -> PathBuf {
    root().join("target/release/ossido")
}

pub fn results_root() -> PathBuf {
    root().join("results")
}

/// Logical core count.
pub fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

// ── ports ────────────────────────────────────────────────────────────────
// Ossido's port is baked into ossido.config.ts at build time; Next's is passed
// via env. Kept off :3000 so a running `ossido dev` doesn't collide.
pub const OSSIDO_PORT: u16 = 4000;
pub const NEXT_PORT: u16 = 4100;

// ── load parameters (env-overridable) ──────────────────────────────────────

/// The env vars that override the default benchmark load.
pub const TUNING_ENV_VARS: &[&str] = &[
    "BENCH_DURATION",
    "BENCH_WARMUP",
    "BENCH_CONNECTIONS",
    "BENCH_PIPELINING",
    "BENCH_TTFB_SAMPLES",
];

fn env_num<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[derive(Clone, Copy)]
pub struct Config {
    pub duration: u64,       // seconds, measured
    pub warmup: u64,         // seconds, discarded
    pub connections: usize,  // concurrent connections
    pub ttfb_samples: usize, // stream-probe samples
    // NB: BENCH_PIPELINING is still an output-gating knob (see TUNING_ENV_VARS)
    // but the load model uses one in-flight request per worker, so it's fixed.
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            duration: env_num("BENCH_DURATION", 10),
            warmup: env_num("BENCH_WARMUP", 3),
            connections: env_num("BENCH_CONNECTIONS", 50),
            ttfb_samples: env_num("BENCH_TTFB_SAMPLES", 30),
        }
    }
}

/// Names of the tuning env vars currently set (a non-default, ad-hoc run).
pub fn overridden_env_vars() -> Vec<&'static str> {
    TUNING_ENV_VARS
        .iter()
        .copied()
        .filter(|k| std::env::var(k).is_ok())
        .collect()
}

/// Whether this run may persist output. Only a default-configuration run writes
/// the canonical `results/<version>/` files and the README table; any tuning
/// override makes the run print-only.
pub fn outputs_enabled() -> bool {
    overridden_env_vars().is_empty()
}

// ── versions ────────────────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
pub struct Versions {
    pub ossido: String,
    pub next: String,
}

fn dep_version(dir: &Path, name: &str) -> Option<String> {
    let raw = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&raw).ok()?;
    for field in ["dependencies", "devDependencies"] {
        if let Some(v) = pkg.get(field).and_then(|d| d.get(name)).and_then(|v| v.as_str()) {
            return Some(v.to_string());
        }
    }
    None
}

/// The dependency versions each example app is pinned to (for JSON output).
pub fn read_versions() -> Versions {
    Versions {
        ossido: dep_version(&ossido_dir(), "@ossido-labs/ossido").unwrap_or_else(|| "unknown".into()),
        next: dep_version(&next_dir(), "next").unwrap_or_else(|| "unknown".into()),
    }
}

/// The Ossido framework version, stripped of any range prefix (`^`, `~`, …).
pub fn ossido_version() -> String {
    read_versions()
        .ossido
        .trim_start_matches(['^', '~', '>', '=', '<', ' '])
        .to_string()
}

/// `results/<ossido-version>/` — path-unsafe chars replaced with `-`.
pub fn results_dir() -> PathBuf {
    let safe: String = ossido_version()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '_' | '-') { c } else { '-' })
        .collect();
    results_root().join(safe)
}

// ── frameworks / thread modes ───────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FrameworkKey {
    Ossido,
    Next,
}

impl FrameworkKey {
    /// Stable machine key used in report maps and JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            FrameworkKey::Ossido => "ossido",
            FrameworkKey::Next => "next",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Framework {
    pub key: FrameworkKey,
    pub label: &'static str,
    pub port: u16,
}

pub const OSSIDO: Framework = Framework { key: FrameworkKey::Ossido, label: "Ossido", port: OSSIDO_PORT };
pub const NEXT: Framework = Framework { key: FrameworkKey::Next, label: "Next.js", port: NEXT_PORT };

pub fn frameworks() -> [Framework; 2] {
    [OSSIDO, NEXT]
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThreadMode {
    Single,
    Multi,
}

impl ThreadMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ThreadMode::Single => "single",
            ThreadMode::Multi => "multi",
        }
    }
    /// Thread/worker count for this mode.
    pub fn threads(self) -> usize {
        match self {
            ThreadMode::Single => 1,
            ThreadMode::Multi => cpu_count(),
        }
    }
}

pub const THREAD_MODES: [ThreadMode; 2] = [ThreadMode::Single, ThreadMode::Multi];

// ── scenarios ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Scenario {
    pub key: &'static str,
    pub title: &'static str,
    pub path: &'static str,
    pub note: &'static str,
    /// Concurrent connections for this scenario. `None` = the configured default.
    pub connections: Option<usize>,
}

/// Load-tested scenarios (throughput + latency). Paths are identical in both
/// apps and render byte-for-byte identical React trees.
pub fn load_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            key: "ssr",
            title: "SSR — catalogue",
            path: "/ssr",
            note: "60 product cards rendered per request",
            connections: None,
        },
        Scenario {
            key: "heavy",
            title: "SSR — heavy table",
            path: "/heavy",
            note: "5000-row table, CPU-bound render",
            // A single ~300ms render saturates a thread on its own; piling on 50
            // connections just builds a queue (and can exceed the request
            // timeout) without measuring more.
            connections: Some(cpu_count() * 2),
        },
        Scenario {
            key: "api",
            title: "JSON API",
            path: "/api/bench",
            note: "100-item JSON payload (Rust vs Node)",
            connections: None,
        },
    ]
}

/// Measured separately for time-to-first-byte vs full response.
pub fn stream_scenario() -> Scenario {
    Scenario {
        key: "stream",
        title: "Streaming SSR",
        path: "/stream",
        note: "shell flushed first, 3000-row table streamed after",
        connections: None,
    }
}

/// Route used for the memory sweep.
pub const MEM_PATH: &str = "/ssr";
