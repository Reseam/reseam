// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::{PatchMetrics, PatchPhase, PatchPhaseMetrics};
use serde::Serialize;

use super::PerfIteration;

#[derive(Debug, Serialize)]
pub(super) struct NumericSummary {
    pub min: u64,
    pub median: u64,
    pub max: u64,
    pub mean: f64,
}

#[derive(Debug, Serialize)]
pub(super) struct PhaseSummary {
    pub phase: PatchPhase,
    pub duration_ms: NumericSummary,
    pub rss_bytes: Option<NumericSummary>,
    pub peak_rss_bytes: Option<NumericSummary>,
}

#[derive(Debug, Serialize)]
pub(super) struct PerfSummary {
    pub successful_iterations: usize,
    pub failed_iterations: usize,
    pub total_duration_ms: Option<NumericSummary>,
    pub final_rss_bytes: Option<NumericSummary>,
    pub peak_rss_bytes: Option<NumericSummary>,
    pub phases: Vec<PhaseSummary>,
}

pub(super) fn summarize(iterations: &[PerfIteration]) -> PerfSummary {
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
