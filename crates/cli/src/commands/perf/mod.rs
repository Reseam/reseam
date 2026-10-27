// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod report;
mod stats;
mod worker;

use anyhow::{Result, ensure};
use serde::Serialize;

use self::report::print_report;
use self::stats::{PerfSummary, summarize};
pub use self::worker::run_perf_worker;
use self::worker::{IterationOutcome, measure};
use crate::app::PerfCommand;

#[derive(Debug, Serialize)]
struct PerfIteration {
    iteration: u32,
    #[serde(flatten)]
    outcome: IterationOutcome,
}

#[derive(Debug, Serialize)]
struct PerfReport {
    apk_path: String,
    bundle_paths: Vec<String>,
    split_count: usize,
    dry_run: bool,
    warmup_iterations: u32,
    measured_iterations: u32,
    iterations: Vec<PerfIteration>,
    summary: PerfSummary,
}

pub fn run_perf(command: &PerfCommand) -> Result<()> {
    ensure!(
        command.iterations > 0,
        "--iterations must be greater than 0"
    );
    for index in 0..command.warmup {
        eprintln!("warmup {}/{}", index + 1, command.warmup);
        if let IterationOutcome::Failure { error } = measure(command)? {
            anyhow::bail!("warmup failed: {error}");
        }
    }
    let iterations = (1..=command.iterations)
        .map(|iteration| {
            eprintln!("iteration {iteration}/{}", command.iterations);
            let outcome = measure(command).unwrap_or_else(|error| IterationOutcome::Failure {
                error: format!("{error:#}"),
            });
            PerfIteration { iteration, outcome }
        })
        .collect::<Vec<_>>();
    let args = &command.request;
    let report = PerfReport {
        apk_path: args.apk.display().to_string(),
        bundle_paths: args
            .bundle
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        split_count: args.split.len(),
        dry_run: args.dry_run,
        warmup_iterations: command.warmup,
        measured_iterations: command.iterations,
        summary: summarize(&iterations),
        iterations,
    };
    if command.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    ensure!(
        report.summary.failed_iterations == 0,
        "one or more performance iterations failed"
    );
    Ok(())
}
