//! Production builds for both example apps (port of `build.ts`) and the
//! `ensure_built` guard shared by the benchmarks.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

use crate::config::{next_dir, ossido_binary, ossido_dir};

fn run(cmd: &str, args: &[&str], cwd: &Path) -> Result<()> {
    let status = Command::new(cmd).args(args).current_dir(cwd).status()?;
    if !status.success() {
        bail!("`{cmd} {}` exited with {status}", args.join(" "));
    }
    Ok(())
}

/// Build both apps: Ossido JS assets + the LTO release binary, then Next.js.
pub fn build_all() -> Result<()> {
    println!("=== Building Ossido: JS assets ===");
    run("bunx", &["ossido", "build", "--server"], &ossido_dir())?;
    println!("\n=== Building Ossido: release binary (cargo, LTO — slow the first time) ===");
    // Run in the example dir so cargo builds just that package, not the workspace.
    run("cargo", &["build", "--release"], &ossido_dir())?;
    println!("\n=== Building Next.js: production ===");
    run("bun", &["run", "build"], &next_dir())?;
    println!("\n✔ Both apps built.");
    Ok(())
}

/// Verify both apps are built; bail with guidance otherwise.
pub fn ensure_built() -> Result<()> {
    let binary = ossido_binary();
    let next_build = next_dir().join(".next");
    let mut problems = Vec::new();
    if !binary.exists() {
        problems.push(format!("missing Ossido binary: {}", binary.display()));
    }
    if !next_build.exists() {
        problems.push(format!("missing Next build: {}", next_build.display()));
    }
    if !problems.is_empty() {
        bail!(
            "Apps are not built:\n  - {}\n\nRun `bun run bench:build` first (or `bench build` / `bench throughput --build`).",
            problems.join("\n  - ")
        );
    }
    Ok(())
}
