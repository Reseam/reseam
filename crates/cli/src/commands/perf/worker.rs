// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::process::{Command, Stdio};

use anyhow::{Context, Result, ensure};
use reseam_model::{PatchMetrics, PatchOutput, PatchRequest};
use reseam_storage::ScratchDir;
use serde::{Deserialize, Serialize};

use crate::app::PerfCommand;
use crate::commands::patch::{request, run as run_patch};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum IterationOutcome {
    Success { metrics: Box<PatchMetrics> },
    Failure { error: String },
}

impl IterationOutcome {
    pub fn metrics(&self) -> Option<&PatchMetrics> {
        match self {
            Self::Success { metrics } => Some(metrics),
            Self::Failure { .. } => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct WorkerRequest {
    request: PatchRequest,
    options: Vec<String>,
}

/// Runs one measured patch in this process, reading the request from stdin
/// and writing the outcome to stdout.
pub fn run_perf_worker() -> Result<()> {
    let WorkerRequest { request, options } = serde_json::from_reader(std::io::stdin().lock())
        .context("read performance worker request")?;
    let outcome = match run_patch(request, &options, |_, _| {}) {
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

/// Runs one patch in a fresh worker process, so runs don't share memory.
pub(super) fn measure(command: &PerfCommand) -> Result<IterationOutcome> {
    let scratch = ScratchDir::new("perf")?;
    let output = PatchOutput::Auto {
        path: scratch.path().join("patched").display().to_string(),
    };
    let message = WorkerRequest {
        request: request(&command.request, output),
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
