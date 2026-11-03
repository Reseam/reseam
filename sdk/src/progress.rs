// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Instant;

use reseam_model::{ApplyDiagnostics, PatchMetrics, PatchPhase, PatchPhaseMetrics, RunEvent};

use crate::metrics::{duration_ms, measure, run_metrics};

/// What a run reports as it goes: the events its host sees, and the time and
/// memory of each phase for the outcome's metrics.
pub(crate) struct RunProgress<'e> {
    emit: &'e mut dyn FnMut(RunEvent),
    started_at: Instant,
    /// Time the run's inputs took to open, before the run started.
    opening_ms: u64,
    phases: Vec<PatchPhaseMetrics>,
    apply_diagnostics: Option<ApplyDiagnostics>,
}

impl<'e> RunProgress<'e> {
    /// Starts with `opening`, the phase that opened the run's inputs.
    pub(crate) fn new(opening: PatchPhaseMetrics, emit: &'e mut dyn FnMut(RunEvent)) -> Self {
        Self {
            emit,
            started_at: Instant::now(),
            opening_ms: opening.duration_ms,
            phases: vec![opening],
            apply_diagnostics: None,
        }
    }

    pub(crate) fn emit(&mut self, event: RunEvent) {
        (self.emit)(event);
    }

    /// Reports `phase` started, then runs it and records its metrics.
    pub(crate) fn measure<T, E>(
        &mut self,
        phase: PatchPhase,
        run: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        self.emit(RunEvent::PhaseStarted { phase });
        let (value, metrics) = measure(phase, || run(&mut *self))?;
        self.phases.push(metrics);
        Ok(value)
    }

    pub(crate) fn set_apply_diagnostics(&mut self, diagnostics: ApplyDiagnostics) {
        self.apply_diagnostics = Some(diagnostics);
    }

    pub(crate) fn finish(self) -> PatchMetrics {
        run_metrics(
            self.opening_ms + duration_ms(self.started_at.elapsed()),
            self.phases,
            self.apply_diagnostics,
        )
    }
}
