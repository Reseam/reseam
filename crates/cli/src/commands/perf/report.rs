// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::ApplyDiagnostics;

use super::PerfReport;
use super::worker::IterationOutcome;

pub(super) fn print_report(report: &PerfReport) {
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
        format_bytes(dex.materialized_ir_bytes)
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
    let accounted = dex.materialized_ir_bytes
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
