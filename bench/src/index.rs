//! Maintains the results index table in README.md (port of `index.ts`). Scans
//! results/<version>/, reads each run's JSON for the framework versions, and
//! rewrites the table between the BENCH_TABLE markers.

use std::fs;

use anyhow::Result;

use crate::config::{results_root, root};

const START: &str = "<!-- BENCH_TABLE:START -->";
const END: &str = "<!-- BENCH_TABLE:END -->";

struct Row {
    ossido: String,
    next: String,
    dir: String,
    has_throughput: bool,
    has_memory: bool,
    generated_at: String,
}

/// Read versions + timestamp from a run dir's results.json or memory.json.
fn read_meta(dir: &std::path::Path) -> (Option<String>, Option<String>, Option<String>) {
    for file in ["results.json", "memory.json"] {
        let p = dir.join(file);
        if !p.exists() {
            continue;
        }
        if let Ok(raw) = fs::read_to_string(&p) {
            if let Ok(j) = serde_json::from_str::<serde_json::Value>(&raw) {
                let ossido = j.get("versions").and_then(|v| v.get("ossido")).and_then(|v| v.as_str()).map(String::from);
                let next = j.get("versions").and_then(|v| v.get("next")).and_then(|v| v.as_str()).map(String::from);
                let generated = j.get("generatedAt").and_then(|v| v.as_str()).map(String::from);
                return (ossido, next, generated);
            }
        }
    }
    (None, None, None)
}

fn build_table() -> String {
    let results = results_root();
    let mut rows: Vec<Row> = Vec::new();
    if let Ok(entries) = fs::read_dir(&results) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir = entry.file_name().to_string_lossy().to_string();
            let (ossido, next, generated) = read_meta(&path);
            rows.push(Row {
                ossido: ossido.unwrap_or_else(|| dir.clone()),
                next: next.unwrap_or_else(|| "—".into()),
                has_throughput: path.join("RESULTS.md").exists(),
                has_memory: path.join("MEMORY.md").exists(),
                generated_at: generated.unwrap_or_default(),
                dir,
            });
        }
    }

    // Newest first (by run timestamp, then version string).
    rows.sort_by(|a, b| b.generated_at.cmp(&a.generated_at).then_with(|| b.ossido.cmp(&a.ossido)));

    if rows.is_empty() {
        return "_No results yet — run `bun run bench` and `bun run bench:memory`._".into();
    }

    let header = "| Ossido version | Next version | Throughput result | Memory result |\n| --- | --- | --- | --- |";
    let body = rows
        .iter()
        .map(|r| {
            let base = format!("./results/{}", r.dir);
            let tp = if r.has_throughput {
                format!("[RESULTS.md]({base}/RESULTS.md) · [json]({base}/results.json)")
            } else {
                "—".into()
            };
            let mem = if r.has_memory {
                format!("[MEMORY.md]({base}/MEMORY.md) · [json]({base}/memory.json)")
            } else {
                "—".into()
            };
            format!("| `{}` | `{}` | {tp} | {mem} |", r.ossido, r.next)
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{header}\n{body}")
}

/// Rewrite the table between the BENCH_TABLE markers in README.md.
pub fn update_readme_table() -> Result<()> {
    let readme = root().join("README.md");
    if !readme.exists() {
        return Ok(());
    }
    let md = fs::read_to_string(&readme)?;
    let (Some(si), Some(ei)) = (md.find(START), md.find(END)) else {
        eprintln!("README.md is missing the {START} / {END} markers — skipping table update.");
        return Ok(());
    };
    if ei < si {
        eprintln!("README.md BENCH_TABLE markers are out of order — skipping table update.");
        return Ok(());
    }
    let block = format!("{START}\n{}\n{END}", build_table());
    let next = format!("{}{}{}", &md[..si], block, &md[ei + END.len()..]);
    if next != md {
        fs::write(&readme, next)?;
    }
    Ok(())
}
