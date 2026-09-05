// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{ensure, Result};
use reseam_apk::scratch::ScratchDir;
use reseam_sdk::{
    patch, ApplyDiagnostics, PatchMetrics, PatchOutput, PatchPhase, PatchPhaseMetrics,
};
use serde::Serialize;

use crate::app::PerfCommand;
use crate::commands::patch::request;

#[derive(Debug, Serialize)]
struct PerfIteration {
    iteration: u32,
    metrics: Option<PatchMetrics>,
    error: Option<String>,
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
    heap_peak_bytes: Option<NumericSummary>,
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
    ensure!(
        command.iterations > 0,
        "--iterations must be greater than 0"
    );
    if let Some(step) = std::env::var("RESEAM_HEAP_TRACE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        reseam_sdk::trace_heap_growth(step << 20);
    }

    let args = &command.request;
    let run = |label: &str| -> Result<PatchMetrics> {
        eprintln!("{label}");
        let scratch = ScratchDir::new("perf")?;
        let output = if args.split.is_empty() {
            PatchOutput::SingleFile {
                path: scratch.path().join("patched.apk"),
            }
        } else {
            PatchOutput::SplitDir {
                path: scratch.path().join("patched"),
            }
        };
        Ok(patch(&request(args, output)?, |_| {})?.metrics)
    };

    for index in 0..command.warmup {
        run(&format!("warmup {}/{}", index + 1, command.warmup))?;
    }
    let iterations: Vec<PerfIteration> = (0..command.iterations)
        .map(|index| {
            let outcome = run(&format!("iteration {}/{}", index + 1, command.iterations));
            PerfIteration {
                iteration: index + 1,
                error: outcome.as_ref().err().map(|error| format!("{error:#}")),
                metrics: outcome.ok(),
            }
        })
        .collect();

    let report = PerfReport {
        apk_path: args.apk.display().to_string(),
        bundle_path: args.bundle.display().to_string(),
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
        .filter_map(|iteration| iteration.metrics.as_ref())
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
        heap_peak_bytes: summarize_values(
            samples.iter().filter_map(|sample| sample.heap_peak_bytes),
        ),
    })
}

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
    println!("Bundle: {}", report.bundle_path);
    println!("Splits: {}", report.split_count);
    println!("Dry run: {}", report.dry_run);
    println!("Warmups: {}", report.warmup_iterations);
    println!("Measured runs: {}", report.measured_iterations);
    println!();

    for iteration in &report.iterations {
        match &iteration.metrics {
            Some(metrics) => println!(
                "iteration {:>2}:      ok  total={}  peak_rss={}  final_rss={} (anon={} file={})  final_heap={}  jvm_committed={}",
                iteration.iteration,
                format_duration(metrics.total_duration_ms),
                format_optional_bytes(metrics.peak_rss_bytes),
                format_optional_bytes(metrics.final_rss_bytes),
                format_optional_bytes(metrics.final_rss_anon_bytes),
                format_optional_bytes(metrics.final_rss_file_bytes),
                format_optional_bytes(metrics.final_heap_live_bytes),
                format_optional_bytes(
                    metrics
                        .apply_diagnostics
                        .as_ref()
                        .and_then(|d| d.jvm.map(|jvm| jvm.committed_bytes))
                ),
            ),
            None => println!(
                "iteration {:>2}:  failed  {}",
                iteration.iteration,
                iteration.error.as_deref().unwrap_or_default()
            ),
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
                "  {:<24} median={} max_peak_rss={} max_peak_heap={}",
                phase.phase.as_str(),
                format_duration(phase.duration_ms.median),
                format_optional_bytes(phase.peak_rss_bytes.as_ref().map(|stats| stats.max)),
                format_optional_bytes(phase.heap_peak_bytes.as_ref().map(|stats| stats.max)),
            );
        }
    }
    if let Some(diagnostics) = report
        .iterations
        .iter()
        .rev()
        .find_map(|iteration| iteration.metrics.as_ref())
        .and_then(|metrics| metrics.apply_diagnostics.as_ref())
    {
        print_apply_diagnostics(diagnostics);
    }
}

fn print_apply_diagnostics(d: &ApplyDiagnostics) {
    let dex = &d.dex;
    let ir = &dex.materialized;
    println!();
    println!("apply_patches memory attribution (sampled at apply-phase peak):");
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
        format_bytes(ir.estimated_ir_bytes())
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
    let accounted = ir.estimated_ir_bytes()
        + dex.raw_buffer_bytes
        + dex.string_pool_bytes
        + dex.id_table_bytes
        + dex.class_def_bytes;
    println!("    sum accounted:           {}", format_bytes(accounted));
    println!();
    match d.jvm {
        Some(jvm) => println!(
            "  jvm heap:                  used={} committed={} max={}",
            format_bytes(jvm.used_bytes),
            format_bytes(jvm.committed_bytes),
            format_bytes(jvm.max_bytes),
        ),
        None => println!("  jvm heap:                  n/a (no live JVM)"),
    }
    if let Some(rss) = d.rss_bytes {
        println!("  rss at apply end:          {}", format_bytes(rss));
        if let Some(committed) = d.jvm.map(|jvm| jvm.committed_bytes) {
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
