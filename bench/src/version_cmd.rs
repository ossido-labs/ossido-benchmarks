//! `version <ver>`: pin the Ossido example to a specific framework version,
//! refresh dependencies, rebuild, and run both benchmarks — all behind a spinner
//! checklist. Port of the old `bench/version.ts`, but the benchmark steps now run
//! in-process so each scenario gets its own live spinner.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

use crate::bench_trait::{BenchCtx, DynBenchmark};
use crate::benchmarks::{Memory, Throughput};
use crate::config::{next_dir, ossido_dir, root};
use crate::ui::Ui;

fn run_quiet(cmd: &str, args: &[&str], cwd: &Path) -> Result<()> {
    let out = Command::new(cmd).args(args).current_dir(cwd).output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let detail = if !stderr.trim().is_empty() { stderr } else { stdout };
        bail!("`{cmd} {}` failed:\n{}", args.join(" "), detail);
    }
    Ok(())
}

/// Rewrite the `ossido = "…"` crate dependency in the example's Cargo.toml.
fn set_cargo_version(version: &str) -> Result<()> {
    let path = ossido_dir().join("Cargo.toml");
    let src = std::fs::read_to_string(&path)?;
    let mut replaced = false;
    let out: Vec<String> = src
        .lines()
        .map(|line| {
            if replaced {
                return line.to_string();
            }
            let trimmed = line.trim_start();
            // Match a bare-string dependency: `ossido = "…"`.
            if let Some(rest) = trimmed.strip_prefix("ossido") {
                let rest = rest.trim_start();
                if let Some(after_eq) = rest.strip_prefix('=') {
                    if after_eq.trim_start().starts_with('"') {
                        replaced = true;
                        let indent = &line[..line.len() - trimmed.len()];
                        return format!("{indent}ossido = \"{version}\"");
                    }
                }
            }
            line.to_string()
        })
        .collect();

    if !replaced {
        bail!("Could not find an `ossido = \"…\"` dependency in {}", path.display());
    }
    let mut new = out.join("\n");
    if src.ends_with('\n') {
        new.push('\n');
    }
    std::fs::write(&path, new)?;
    Ok(())
}

/// Set every @ossido-labs/* dependency in the example's package.json.
fn set_npm_version(version: &str) -> Result<usize> {
    let path = ossido_dir().join("package.json");
    let raw = std::fs::read_to_string(&path)?;
    // Needs serde_json's `preserve_order` so we don't scramble the manifest.
    let mut pkg: serde_json::Value = serde_json::from_str(&raw)?;
    let mut updated = 0;
    for field in ["dependencies", "devDependencies"] {
        if let Some(deps) = pkg.get_mut(field).and_then(|v| v.as_object_mut()) {
            let names: Vec<String> =
                deps.keys().filter(|k| k.starts_with("@ossido-labs/")).cloned().collect();
            for name in names {
                deps.insert(name, serde_json::Value::String(version.to_string()));
                updated += 1;
            }
        }
    }
    if updated == 0 {
        bail!("No @ossido-labs/* dependencies found in {}", path.display());
    }
    let mut out = serde_json::to_string_pretty(&pkg)?;
    out.push('\n');
    std::fs::write(&path, out)?;
    Ok(updated)
}

pub async fn run_version(version: &str) -> Result<()> {
    let ui = Ui::new();
    println!("\nBenchmarking Ossido {version}\n");

    // Step 1 — pin the crate.
    let h = ui.step(&format!("Setting Ossido crate to {version}"));
    match set_cargo_version(version) {
        Ok(()) => h.finish_ok("examples/ossido/Cargo.toml"),
        Err(e) => {
            h.finish_err(&e.to_string());
            return Err(e);
        }
    }

    // Step 2 — resolve + download crates (cannot `cargo update -p ossido`: the
    // example package is itself named `ossido`, so the spec is ambiguous).
    let h = ui.step("Generating Ossido project (cargo fetch)");
    match run_quiet("cargo", &["fetch"], &ossido_dir()) {
        Ok(()) => h.finish_ok("crates resolved"),
        Err(e) => {
            h.finish_err(&e.to_string());
            return Err(e);
        }
    }

    // Step 3 — pin npm packages + install.
    let h = ui.step(&format!("Setting Ossido packages to {version}"));
    let step3: Result<usize> = (|| {
        let n = set_npm_version(version)?;
        run_quiet("bun", &["install"], &root())?;
        Ok(n)
    })();
    match step3 {
        Ok(n) => h.finish_ok(&format!("{n} packages · bun install")),
        Err(e) => {
            h.finish_err(&e.to_string());
            return Err(e);
        }
    }

    // Step 4 — build both apps (quiet; the LTO cargo build is slow).
    let h = ui.step("Building apps (ossido build + cargo --release + next build)");
    let build: Result<()> = (|| {
        run_quiet("bunx", &["ossido", "build", "--server"], &ossido_dir())?;
        run_quiet("cargo", &["build", "--release"], &ossido_dir())?;
        run_quiet("bun", &["run", "build"], &next_dir())?;
        Ok(())
    })();
    match build {
        Ok(()) => h.finish_ok("both apps built"),
        Err(e) => {
            h.finish_err(&e.to_string());
            return Err(e);
        }
    }

    // Steps 5 & 6 — run both benchmarks in-process (each shows per-scenario
    // spinners). No BENCH_* overrides here, so results land in results/<version>/.
    let ctx = BenchCtx::new()?;
    for bench in [Box::new(Throughput) as Box<dyn DynBenchmark>, Box::new(Memory)] {
        let progress = ui.benchmark(bench.name());
        bench.run_and_finish(&ctx, &progress).await?;
        progress.finish(&format!("{} — done", bench.name()));
    }

    println!("\n✔ Done — results for {version} are in results/");
    Ok(())
}
