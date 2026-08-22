//! Framework server lifecycle: spawn detached (own process group via `setsid`,
//! equivalent to Node's `detached: true`), poll for readiness, and SIGKILL the
//! whole process group on stop (so Node cluster workers / Ossido threads all die).

use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

use crate::config::{
    next_dir, ossido_binary, ossido_dir, Framework, FrameworkKey, NEXT_PORT, OSSIDO_PORT,
};

pub struct RunningServer {
    // Retained for context/debugging even though the sweep only reads `pgid`.
    #[allow(dead_code)]
    pub framework: Framework,
    #[allow(dead_code)]
    pub threads: usize,
    /// Process-group id of the server tree (= the spawned child's pid, since it
    /// is started as its own session/group leader). Used to sum RSS and to kill.
    pub pgid: i32,
    child: Child,
}

impl RunningServer {
    /// SIGKILL the whole process group, then reap the leader.
    pub async fn stop(self) -> Result<()> {
        let RunningServer { mut child, pgid, .. } = self;
        // Negative pid targets the entire process group.
        let _ = kill(Pid::from_raw(-pgid), Signal::SIGKILL);
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            tokio::task::spawn_blocking(move || {
                let _ = child.wait();
            }),
        )
        .await;
        Ok(())
    }
}

fn spawn_detached(mut cmd: Command) -> Result<Child> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // SAFETY: `setsid` is async-signal-safe and the only thing we do in the
    // child between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid()
                .map(|_| ())
                .map_err(|e| std::io::Error::from_raw_os_error(e as i32))
        });
    }
    Ok(cmd.spawn()?)
}

async fn wait_for_server(http: &reqwest::Client, url: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(resp) = http.get(url).send().await {
            if resp.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err(anyhow!("Server at {url} did not become ready within {timeout:?}"))
}

async fn finish_start(
    http: &reqwest::Client,
    framework: Framework,
    threads: usize,
    child: Child,
    health_url: String,
) -> Result<RunningServer> {
    let pgid = child.id() as i32;
    let server = RunningServer { framework, threads, pgid, child };
    if let Err(e) = wait_for_server(http, &health_url, Duration::from_secs(30)).await {
        let _ = server.stop().await;
        return Err(e);
    }
    Ok(server)
}

async fn start_ossido(http: &reqwest::Client, framework: Framework, threads: usize) -> Result<RunningServer> {
    let mut cmd = Command::new(ossido_binary());
    cmd.current_dir(ossido_dir());
    cmd.env("OSSIDO_SSR_THREADS", threads.to_string());
    let child = spawn_detached(cmd)?;
    finish_start(http, framework, threads, child, format!("http://127.0.0.1:{OSSIDO_PORT}/ssr")).await
}

async fn start_next(http: &reqwest::Client, framework: Framework, workers: usize) -> Result<RunningServer> {
    let mut cmd = Command::new("node");
    cmd.arg("server.mjs");
    cmd.current_dir(next_dir());
    cmd.env("NODE_ENV", "production");
    cmd.env("PORT", NEXT_PORT.to_string());
    cmd.env("WEB_CONCURRENCY", workers.to_string());
    let child = spawn_detached(cmd)?;
    finish_start(http, framework, workers, child, format!("http://127.0.0.1:{NEXT_PORT}/ssr")).await
}

/// Start a framework's server with an explicit thread/worker count.
pub async fn start_server(http: &reqwest::Client, framework: Framework, threads: usize) -> Result<RunningServer> {
    match framework.key {
        FrameworkKey::Ossido => start_ossido(http, framework, threads).await,
        FrameworkKey::Next => start_next(http, framework, threads).await,
    }
}
