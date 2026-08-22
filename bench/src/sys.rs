//! System/environment introspection: host string, CPU model, total memory, an
//! ISO-8601 timestamp, and process-group RSS sampling (replaces the `ps` parsing
//! from `memory.ts`).

use nix::unistd::{getpgid, Pid};
use sysinfo::{ProcessesToUpdate, System};

/// One-shot environment snapshot for report headers.
pub struct EnvInfo {
    pub host: String,
    pub cpu_model: String,
    pub total_mem_gb: f64,
}

pub fn env_info() -> EnvInfo {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let name = System::name().unwrap_or_else(|| "unknown".into());
    let kernel = System::kernel_version().unwrap_or_default();
    let host = format!("{name} {kernel} · {}", std::env::consts::ARCH);

    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());

    let total_mem_gb = round1(sys.total_memory() as f64 / 1024f64.powi(3));

    EnvInfo { host, cpu_model, total_mem_gb }
}

/// ISO-8601 / RFC-3339 timestamp (e.g. `2026-08-22T05:14:00Z`).
pub fn now_iso() -> String {
    jiff::Timestamp::now().to_string()
}

/// Sum RSS (MB) of every process in the given process group. Reuses a long-lived
/// `System` so repeated 250ms samples are cheap.
pub fn group_rss_mb(sys: &mut System, pgid: i32) -> f64 {
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut bytes = 0u64;
    for (pid, process) in sys.processes() {
        let raw = pid.as_u32() as i32;
        if let Ok(pg) = getpgid(Some(Pid::from_raw(raw))) {
            if pg.as_raw() == pgid {
                bytes += process.memory();
            }
        }
    }
    bytes as f64 / (1024.0 * 1024.0)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
