// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::{Command, Stdio};

use anyhow::{Context, Result, ensure};
use reseam_sdk::{ApplyDiagnostics, PatchMetrics, PatchOutput, PatchPhase, PatchPhaseMetrics};
use reseam_storage::ScratchDir;
use serde::{Deserialize, Serialize};

use crate::app::PerfCommand;
use crate::commands::patch::{request, run as run_patch};

#[derive(Debug, Serialize)]
struct PerfIteration {
    iteration: u32,
    #[serde(flatten)]
    outcome: IterationOutcome,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum IterationOutcome {
    Success { metrics: Box<PatchMetrics> },
    Failure { error: String },
}

impl IterationOutcome {
    fn metrics(&self) -> Option<&PatchMetrics> {
        match self {
            Self::Success { metrics } => Some(metrics),
            Self::Failure { .. } => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct WorkerRequest {
    request: reseam_sdk::PatchRequest,
    options: Vec<String>,
}

pub fn run_perf_worker() -> Result<()> {
    let WorkerRequest { request, options } = serde_json::from_reader(std::io::stdin().lock())
        .context("read performance worker request")?;
    let outcome = match run_patch(&request, &options, |_, _| {}) {
        Ok(outcome) => IterationOutcome::Success {
            metrics: Box::new(outcome.metrics),
        },
        Err(error) => IterationOutcome::Failure {
            error: format!("{error:#}"),
        },
    };
    serde_json::to_writer(std::io::stdout().lock(), &outcome)
        .context("write performance worker result")
}

fn measure(command: &PerfCommand) -> Result<IterationOutcome> {
    let scratch = ScratchDir::new("perf")?;
    let output = PatchOutput::Auto {
        path: scratch.path().join("patched").display().to_string(),
    };
    let message = WorkerRequest {
        request: request(&command.request, output)?,
        options: command.request.option.clone(),
    };
    let mut child = Command::new(std::env::current_exe()?)
        .arg("perf-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("start performance worker")?;
    let sent = serde_json::to_writer(
        child.stdin.take().context("worker stdin was piped")?,
        &message,
    );
    // Reap even if the worker exits before consuming its request.
    let output = child
        .wait_with_output()
        .context("wait for performance worker")?;
    sent.context("send performance worker request")?;
    ensure!(
        output.status.success(),
        "performance worker exited with {}",
        output.status
    );
    serde_json::from_slice(&output.stdout).context("read performance worker result")
}

#[derive(Debug, Serialize)]
struct NumericSummary {
    min: u64,
    median: u64,
    max: u64,
    mean: f64,
}

#[derive(Debug, Serialize)]
struct PhaseSummary {
    phase: PatchPhase,
    duration_ms: NumericSummary,
    rss_bytes: Option<NumericSummary>,
    peak_rss_bytes: Option<NumericSummary>,
}

#[derive(Debug, Serialize)]
struct PerfSummary {
    successful_iterations: usize,
    failed_iterations: usize,
    total_duration_ms: Option<NumericSummary>,
    final_rss_bytes: Option<NumericSummary>,
    peak_rss_bytes: Option<NumericSummary>,
    phases: Vec<PhaseSummary>,
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

fn summarize(iterations: &[PerfIteration]) -> PerfSummary {
    let successful: Vec<&PatchMetrics> = iterations
        .iter()
        .filter_map(|iteration| iteration.outcome.metrics())
        .collect();
    let phases = successful
        .first()
        .map(|metrics| metrics.phases.iter().map(|sample| sample.phase))
        .into_iter()
        .flatten()
        .filter_map(|phase| summarize_phase(phase, &successful))
        .collect();
    PerfSummary {
        successful_iterations: successful.len(),
        failed_iterations: iterations.len() - successful.len(),
        total_duration_ms: summarize_values(successful.iter().map(|m| m.total_duration_ms)),
        final_rss_bytes: summarize_values(successful.iter().filter_map(|m| m.final_rss_bytes)),
        peak_rss_bytes: summarize_values(successful.iter().filter_map(|m| m.peak_rss_bytes)),
        phases,
    }
}

fn summarize_phase(phase: PatchPhase, iterations: &[&PatchMetrics]) -> Option<PhaseSummary> {
    let samples: Vec<&PatchPhaseMetrics> = iterations
        .iter()
        .filter_map(|metrics| metrics.phases.iter().find(|sample| sample.phase == phase))
        .collect();
    Some(PhaseSummary {
        phase,
        duration_ms: summarize_values(samples.iter().map(|sample| sample.duration_ms))?,
        rss_bytes: summarize_values(samples.iter().filter_map(|sample| sample.rss_bytes)),
        peak_rss_bytes: summarize_values(samples.iter().filter_map(|sample| sample.peak_rss_bytes)),
    })
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Statistical means intentionally round to floating point"
)]
fn summarize_values(values: impl IntoIterator<Item = u64>) -> Option<NumericSummary> {
    let mut values: Vec<u64> = values.into_iter().collect();
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let sum: u128 = values.iter().map(|value| u128::from(*value)).sum();
    Some(NumericSummary {
        min: values[0],
        median: values[values.len() / 2],
        max: values[values.len() - 1],
        mean: sum as f64 / values.len() as f64,
    })
}

fn print_report(report: &PerfReport) {
    println!("APK: {}", report.apk_path);
    println!("Bundles: {}", report.bundle_paths.join(", "));
    println!("Splits: {}", report.split_count);
    println!("Dry run: {}", report.dry_run);
    println!("Warmups: {}", report.warmup_iterations);
    println!("Measured runs: {}", report.measured_iterations);
    println!();

    for iteration in &report.iterations {
        match &iteration.outcome {
            IterationOutcome::Success { metrics } => println!(
                "iteration {:>2}:      ok  total={}  process_peak_rss={}  final_rss={} (anon={} file={})  jvm_committed={}",
                iteration.iteration,
                format_duration(metrics.total_duration_ms),
                format_optional_bytes(metrics.peak_rss_bytes),
                format_optional_bytes(metrics.final_rss_bytes),
                format_optional_bytes(metrics.final_rss_anon_bytes),
                format_optional_bytes(metrics.final_rss_file_bytes),
                format_optional_bytes(
                    metrics
                        .apply_diagnostics
                        .as_ref()
                        .and_then(|d| d.jvm.map(|jvm| jvm.committed))
                ),
            ),
            IterationOutcome::Failure { error } => {
                println!("iteration {:>2}:  failed  {error}", iteration.iteration);
            }
        }
    }

    println!();
    println!(
        "summary: {} ok, {} failed",
        report.summary.successful_iterations, report.summary.failed_iterations
    );
    if let Some(total) = &report.summary.total_duration_ms {
        println!(
            "  total: min={} median={} max={} mean={:.1} ms",
            format_duration(total.min),
            format_duration(total.median),
            format_duration(total.max),
            total.mean,
        );
    }
    if let Some(peak) = &report.summary.peak_rss_bytes {
        println!(
            "  process peak rss: min={} median={} max={}",
            format_bytes(peak.min),
            format_bytes(peak.median),
            format_bytes(peak.max),
        );
    }
    if !report.summary.phases.is_empty() {
        println!();
        println!("phase breakdown:");
        for phase in &report.summary.phases {
            println!(
                "  {:<24} median={} max_endpoint_rss={} process_peak_at_end={}",
                phase.phase.as_str(),
                format_duration(phase.duration_ms.median),
                format_optional_bytes(phase.rss_bytes.as_ref().map(|stats| stats.max)),
                format_optional_bytes(phase.peak_rss_bytes.as_ref().map(|stats| stats.max)),
            );
        }
    }
    if let Some(diagnostics) = report
        .iterations
        .iter()
        .rev()
        .find_map(|iteration| iteration.outcome.metrics())
        .and_then(|metrics| metrics.apply_diagnostics.as_ref())
    {
        print_apply_diagnostics(diagnostics);
    }
}

fn print_apply_diagnostics(d: &ApplyDiagnostics) {
    let dex = &d.dex;
    let ir = &dex.materialized;
    println!();
    println!("apply_patches memory attribution (sampled at apply end):");
    println!(
        "  materialized classes:      {} / {}",
        ir.resolved_classes, ir.total_classes
    );
    println!("  materialized methods:      {}", ir.methods);
    println!("  materialized instructions: {}", ir.instructions);
    println!();
    println!("  native heap attribution (all lower bounds):");
    println!(
        "    materialized IR:         {}",
        format_bytes(estimated_ir_bytes(ir))
    );
    println!(
        "    raw dex buffers:         {}",
        format_bytes(dex.raw_buffer_bytes)
    );
    println!(
        "    string pool:             {} ({} strings)",
        format_bytes(dex.string_pool_bytes),
        dex.string_count
    );
    println!(
        "    id tables:               {}",
        format_bytes(dex.id_table_bytes)
    );
    println!(
        "    class-def structs:       {}",
        format_bytes(dex.class_def_bytes)
    );
    let accounted = estimated_ir_bytes(ir)
        + dex.raw_buffer_bytes
        + dex.string_pool_bytes
        + dex.id_table_bytes
        + dex.class_def_bytes;
    println!("    sum accounted:           {}", format_bytes(accounted));
    println!();
    match d.jvm {
        Some(jvm) => println!(
            "  jvm heap:                  used={} committed={} max={}",
            format_bytes(jvm.used),
            format_bytes(jvm.committed),
            format_bytes(jvm.max),
        ),
        None => println!("  jvm heap:                  n/a (no live JVM)"),
    }
    if let Some(rss) = d.rss_bytes {
        println!("  rss at apply end:          {}", format_bytes(rss));
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Human-readable durations intentionally round to hundredths of seconds"
)]
fn format_duration(duration_ms: u64) -> String {
    if duration_ms >= 1000 {
        format!("{:.2}s", duration_ms as f64 / 1000.0)
    } else {
        format!("{duration_ms}ms")
    }
}

fn format_optional_bytes(bytes: Option<u64>) -> String {
    bytes.map_or_else(|| "n/a".to_string(), format_bytes)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Human-readable memory sizes intentionally round to hundredths of a unit"
)]
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes}{}", UNITS[unit])
    } else {
        format!("{value:.2}{}", UNITS[unit])
    }
}

fn estimated_ir_bytes(stats: &reseam_model::MaterializationStats) -> u64 {
    reseam_apk::reseam_dex::estimated_ir_bytes(&reseam_apk::reseam_dex::MaterializationStats {
        total_classes: stats.total_classes,
        resolved_classes: stats.resolved_classes,
        methods: stats.methods,
        instructions: stats.instructions,
    })
}
