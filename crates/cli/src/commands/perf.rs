// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{bail, Result};
use reseam_sdk::{ApplyDiagnostics, measure_patch, PatchMetrics, PatchPhase, PatchPhaseMetrics};
use serde::Serialize;

use crate::app::PerfCommand;
use crate::commands::patch::build_patch_request;

const PHASE_ORDER: [PatchPhase; 8] = [
    PatchPhase::OpenApk,
    PatchPhase::LoadBundles,
    PatchPhase::CompileSelection,
    PatchPhase::ValidatePatches,
    PatchPhase::ApplyPatches,
    PatchPhase::WriteUnsignedArtifacts,
    PatchPhase::LoadSigningKey,
    PatchPhase::SignArtifacts,
];

#[derive(Debug, Serialize)]
struct PerfIteration {
    iteration: u32,
    success: bool,
    error: Option<String>,
    metrics: PatchMetrics,
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
    bundle_path: String,
    split_count: usize,
    dry_run: bool,
    warmup_iterations: u32,
    measured_iterations: u32,
    iterations: Vec<PerfIteration>,
    summary: PerfSummary,
}

pub fn run_perf(command: &PerfCommand) -> Result<()> {
    if command.iterations == 0 {
        bail!("--iterations must be greater than 0");
    }

    let mut iterations = Vec::with_capacity(command.iterations as usize);

    for warmup_index in 0..command.warmup {
        eprintln!("warmup {}/{}", warmup_index + 1, command.warmup);
        let _temp_dir = tempfile::tempdir()?;
        let output = perf_output(&command.request.split, _temp_dir.path());
        let request = build_patch_request(&command.request, output)?;
        let report = measure_patch(&request, |_| {});
        if let Err(error) = report.outcome {
            bail!("warmup iteration {} failed: {error:#}", warmup_index + 1);
        }
    }

    for iteration_index in 0..command.iterations {
        eprintln!("iteration {}/{}", iteration_index + 1, command.iterations);
        let temp_dir = tempfile::tempdir()?;
        let output = perf_output(&command.request.split, temp_dir.path());
        let request = build_patch_request(&command.request, output)?;
        let report = measure_patch(&request, |_| {});

        let (success, error) = match report.outcome {
            Ok(_) => (true, None),
            Err(error) => (false, Some(format!("{error:#}"))),
        };

        iterations.push(PerfIteration {
            iteration: iteration_index + 1,
            success,
            error,
            metrics: report.metrics,
        });
    }

    let summary = summarize_iterations(&iterations);
    let perf_report = PerfReport {
        apk_path: command.request.apk.display().to_string(),
        bundle_path: command.request.bundle.display().to_string(),
        split_count: command.request.split.len(),
        dry_run: command.request.dry_run,
        warmup_iterations: command.warmup,
        measured_iterations: command.iterations,
        iterations,
        summary,
    };

    if command.json {
        println!("{}", serde_json::to_string_pretty(&perf_report)?);
    } else {
        print_report(&perf_report);
    }

    if perf_report.summary.failed_iterations > 0 {
        bail!("one or more performance iterations failed");
    }

    Ok(())
}

fn perf_output(
    split_paths: &[std::path::PathBuf],
    temp_dir: &std::path::Path,
) -> reseam_sdk::PatchOutput {
    if split_paths.is_empty() {
        reseam_sdk::PatchOutput::SingleFile(temp_dir.join("patched.apk"))
    } else {
        reseam_sdk::PatchOutput::SplitDir(temp_dir.join("patched"))
    }
}

fn summarize_iterations(iterations: &[PerfIteration]) -> PerfSummary {
    let successful: Vec<&PerfIteration> = iterations
        .iter()
        .filter(|iteration| iteration.success)
        .collect();
    let failed_iterations = iterations.len().saturating_sub(successful.len());

    let phases = PHASE_ORDER
        .into_iter()
        .filter_map(|phase| summarize_phase(phase, &successful))
        .collect();

    PerfSummary {
        successful_iterations: successful.len(),
        failed_iterations,
        total_duration_ms: summarize_u64(
            successful
                .iter()
                .map(|iteration| iteration.metrics.total_duration_ms),
        ),
        final_rss_bytes: summarize_optional_u64(
            successful
                .iter()
                .filter_map(|iteration| iteration.metrics.final_rss_bytes),
        ),
        peak_rss_bytes: summarize_optional_u64(
            successful
                .iter()
                .filter_map(|iteration| iteration.metrics.peak_rss_bytes),
        ),
        phases,
    }
}

fn summarize_phase(phase: PatchPhase, iterations: &[&PerfIteration]) -> Option<PhaseSummary> {
    let samples: Vec<&PatchPhaseMetrics> = iterations
        .iter()
        .filter_map(|iteration| {
            iteration
                .metrics
                .phases
                .iter()
                .find(|sample| sample.phase == phase)
        })
        .collect();

    if samples.is_empty() {
        return None;
    }

    Some(PhaseSummary {
        phase,
        duration_ms: summarize_u64(samples.iter().map(|sample| sample.duration_ms))
            .expect("phase stats"),
        rss_bytes: summarize_optional_u64(samples.iter().filter_map(|sample| sample.rss_bytes)),
        peak_rss_bytes: summarize_optional_u64(
            samples.iter().filter_map(|sample| sample.peak_rss_bytes),
        ),
    })
}

fn summarize_u64(values: impl IntoIterator<Item = u64>) -> Option<NumericSummary> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }

    values.sort_unstable();
    let min = values[0];
    let max = values[values.len() - 1];
    let median = values[values.len() / 2];
    let sum: u128 = values.iter().map(|value| u128::from(*value)).sum();
    let mean = sum as f64 / values.len() as f64;

    Some(NumericSummary {
        min,
        median,
        max,
        mean,
    })
}

fn summarize_optional_u64(values: impl IntoIterator<Item = u64>) -> Option<NumericSummary> {
    summarize_u64(values)
}

fn print_report(report: &PerfReport) {
    println!("APK: {}", report.apk_path);
    println!("Bundle: {}", report.bundle_path);
    println!("Splits: {}", report.split_count);
    println!("Dry run: {}", report.dry_run);
    println!("Warmups: {}", report.warmup_iterations);
    println!("Measured runs: {}", report.measured_iterations);
    println!();

    for iteration in &report.iterations {
        let status = if iteration.success { "ok" } else { "failed" };
        println!(
            "iteration {:>2}: {:>7}  total={}  peak_rss={}  final_rss={}",
            iteration.iteration,
            status,
            format_duration(iteration.metrics.total_duration_ms),
            format_optional_bytes(iteration.metrics.peak_rss_bytes),
            format_optional_bytes(iteration.metrics.final_rss_bytes),
        );
        if let Some(error) = &iteration.error {
            println!("  error: {error}");
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
            "  peak rss: min={} median={} max={}",
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
                "  {:<24} median={} max_peak={}",
                phase.phase.as_str(),
                format_duration(phase.duration_ms.median),
                phase
                    .peak_rss_bytes
                    .as_ref()
                    .map(|stats| format_bytes(stats.max))
                    .unwrap_or_else(|| "n/a".to_string()),
            );
        }
    }

    if let Some(diagnostics) = report
        .iterations
        .iter()
        .rev()
        .find(|iteration| iteration.success)
        .and_then(|iteration| iteration.metrics.apply_diagnostics.as_ref())
    {
        print_apply_diagnostics(diagnostics);
    }
}

fn print_apply_diagnostics(d: &ApplyDiagnostics) {
    println!();
    println!("apply_patches memory attribution (sampled at apply-phase peak):");
    println!(
        "  materialized classes:      {} / {}",
        d.resolved_classes, d.total_classes
    );
    println!("  materialized methods:      {}", d.materialized_methods);
    println!(
        "  materialized instructions: {}",
        d.materialized_instructions
    );
    println!();
    println!("  native heap attribution (all lower bounds):");
    println!(
        "    materialized IR:         {}",
        format_bytes(d.estimated_ir_bytes)
    );
    println!(
        "    raw dex buffers:         {}",
        format_bytes(d.raw_buffer_bytes)
    );
    println!(
        "    string pool:             {} ({} strings)",
        format_bytes(d.string_pool_bytes),
        d.string_count
    );
    println!(
        "    id tables:               {}",
        format_bytes(d.id_table_bytes)
    );
    println!(
        "    class-def structs:       {}",
        format_bytes(d.class_def_bytes)
    );
    let accounted = d.estimated_ir_bytes
        + d.raw_buffer_bytes
        + d.string_pool_bytes
        + d.id_table_bytes
        + d.class_def_bytes;
    println!("    sum accounted:           {}", format_bytes(accounted));
    println!();
    match (d.jvm_used_bytes, d.jvm_committed_bytes) {
        (Some(used), Some(committed)) => println!(
            "  jvm heap:                  used={} committed={} max={}",
            format_bytes(used),
            format_bytes(committed),
            d.jvm_max_bytes
                .map(format_bytes)
                .unwrap_or_else(|| "n/a".to_string()),
        ),
        _ => println!("  jvm heap:                  n/a (no live JVM)"),
    }
    if let Some(rss) = d.rss_bytes {
        println!("  rss at apply end:          {}", format_bytes(rss));
        if let Some(committed) = d.jvm_committed_bytes {
            let native = rss.saturating_sub(committed);
            println!("  -> native (rss - jvm):     {}", format_bytes(native));
            println!(
                "  -> unaccounted (frag/etc): {}",
                format_bytes(native.saturating_sub(accounted))
            );
        }
    }
}

fn format_duration(duration_ms: u64) -> String {
    if duration_ms >= 1000 {
        format!("{:.2}s", duration_ms as f64 / 1000.0)
    } else {
        format!("{duration_ms}ms")
    }
}

fn format_optional_bytes(bytes: Option<u64>) -> String {
    bytes.map(format_bytes).unwrap_or_else(|| "n/a".to_string())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
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
