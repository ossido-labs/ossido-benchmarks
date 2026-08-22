//! Live progress UI: a MultiProgress with one parent spinner per benchmark and
//! one child spinner per scenario. The child spinners are the fix for "no
//! spinners for individual tests" — every scenario animates while its (blocking)
//! load test runs, via indicatif's steady tick. Falls back to plain lines when
//! stderr is not a TTY.

use std::io::IsTerminal;
use std::time::Duration;

use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

/// Root UI for a whole run. Hand each benchmark its own [`Progress`].
pub struct Ui {
    multi: MultiProgress,
    tty: bool,
}

impl Ui {
    pub fn new() -> Self {
        let tty = std::io::stderr().is_terminal();
        let multi = MultiProgress::new();
        if !tty {
            multi.set_draw_target(ProgressDrawTarget::hidden());
        }
        Ui { multi, tty }
    }

    /// Start a top-level group for one benchmark (its scenarios nest beneath it).
    pub fn benchmark(&self, name: &str) -> Progress {
        let parent = self.multi.add(ProgressBar::new_spinner());
        parent.set_style(parent_style());
        parent.set_prefix(name.to_string());
        parent.enable_steady_tick(Duration::from_millis(80));
        if !self.tty {
            eprintln!("▶ {name}");
        }
        Progress { multi: self.multi.clone(), parent, tty: self.tty }
    }

    /// A free-standing step spinner (used by the version checklist for the
    /// pin/fetch/install steps that aren't scenarios).
    pub fn step(&self, label: &str) -> ScenarioHandle {
        if self.tty {
            let bar = self.multi.add(ProgressBar::new_spinner());
            bar.set_style(step_style());
            bar.set_message(label.to_string());
            bar.enable_steady_tick(Duration::from_millis(80));
            ScenarioHandle { bar: Some(bar), label: label.to_string() }
        } else {
            eprintln!("… {label}");
            ScenarioHandle { bar: None, label: label.to_string() }
        }
    }
}

/// A benchmark's progress handle: updates its parent line and spawns per-scenario
/// child spinners. Cheap to clone (indicatif handles are `Arc` inside).
#[derive(Clone)]
pub struct Progress {
    multi: MultiProgress,
    parent: ProgressBar,
    tty: bool,
}

impl Progress {
    /// Update the benchmark's parent line, e.g. "Ossido · multi (10 threads)".
    pub fn set_stage(&self, msg: &str) {
        if self.tty {
            self.parent.set_message(msg.to_string());
        } else {
            eprintln!("  · {msg}");
        }
    }

    /// Register a child spinner nested under this benchmark for one scenario.
    pub fn scenario(&self, label: &str) -> ScenarioHandle {
        if self.tty {
            let bar = self.multi.insert_after(&self.parent, ProgressBar::new_spinner());
            bar.set_style(child_style());
            bar.set_message(label.to_string());
            bar.enable_steady_tick(Duration::from_millis(80));
            ScenarioHandle { bar: Some(bar), label: label.to_string() }
        } else {
            eprintln!("    … {label}");
            ScenarioHandle { bar: None, label: label.to_string() }
        }
    }

    /// Finish the benchmark's parent line.
    pub fn finish(&self, msg: &str) {
        if self.tty {
            self.parent.set_style(parent_done_style());
            self.parent.finish_with_message(msg.to_string());
        } else {
            eprintln!("✓ {msg}");
        }
    }
}

/// A single running step/scenario spinner.
pub struct ScenarioHandle {
    bar: Option<ProgressBar>,
    label: String,
}

impl ScenarioHandle {
    /// Update the live message while the scenario runs.
    #[allow(dead_code)]
    pub fn tick(&self, msg: &str) {
        if let Some(bar) = &self.bar {
            bar.set_message(format!("{} · {msg}", self.label));
        }
    }

    pub fn finish_ok(&self, summary: &str) {
        let line = format!("{} {} — {}", "✓".green(), self.label, summary.dimmed());
        match &self.bar {
            Some(bar) => {
                bar.set_style(msg_style());
                bar.finish_with_message(line);
            }
            None => eprintln!("    {line}"),
        }
    }

    pub fn finish_err(&self, err: &str) {
        let line = format!("{} {} — {}", "✗".red(), self.label, err.red());
        match &self.bar {
            Some(bar) => {
                bar.set_style(msg_style());
                bar.finish_with_message(line);
            }
            None => eprintln!("    {line}"),
        }
    }
}

fn parent_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan.bold} {prefix:.bold} {msg}")
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
}

fn parent_done_style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:.green.bold} {msg}").unwrap()
}

fn child_style() -> ProgressStyle {
    ProgressStyle::with_template("    {spinner:.cyan} {msg:.dim}")
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
}

fn step_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {msg}")
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
}

/// A finished line: just the (already-formatted) message, no spinner.
fn msg_style() -> ProgressStyle {
    ProgressStyle::with_template("{msg}").unwrap()
}
