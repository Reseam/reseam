// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::time::Instant;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchPhase {
    OpenApk,
    LoadBundles,
    CompileSelection,
    ValidatePatches,
    ApplyPatches,
    WriteUnsignedArtifacts,
    LoadSigningKey,
    SignArtifacts,
}

impl PatchPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenApk => "open_apk",
            Self::LoadBundles => "load_bundles",
            Self::CompileSelection => "compile_selection",
            Self::ValidatePatches => "validate_patches",
            Self::ApplyPatches => "apply_patches",
            Self::WriteUnsignedArtifacts => "write_unsigned_artifacts",
            Self::LoadSigningKey => "load_signing_key",
            Self::SignArtifacts => "sign_artifacts",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchPhaseMetrics {
    pub phase: PatchPhase,
    pub duration_ms: u64,
    pub rss_bytes: Option<u64>,
    pub peak_rss_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatchMetrics {
    pub total_duration_ms: u64,
    pub final_rss_bytes: Option<u64>,
    pub peak_rss_bytes: Option<u64>,
    pub phases: Vec<PatchPhaseMetrics>,
}

#[derive(Debug)]
pub struct PatchExecutionReport {
    pub outcome: anyhow::Result<crate::dto::PatchOutcome>,
    pub metrics: PatchMetrics,
}

#[derive(Debug, Clone, Copy, Default)]
struct MemorySample {
    rss_bytes: Option<u64>,
    peak_rss_bytes: Option<u64>,
}

pub(crate) struct PatchProfiler {
    started_at: Instant,
    phases: Vec<PatchPhaseMetrics>,
}

impl PatchProfiler {
    pub(crate) fn new() -> Self {
        Self {
            started_at: Instant::now(),
            phases: Vec::new(),
        }
    }

    pub(crate) fn measure<T, E>(
        &mut self,
        phase: PatchPhase,
        run: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let phase_started_at = Instant::now();
        let result = run();
        let memory = sample_memory();
        self.phases.push(PatchPhaseMetrics {
            phase,
            duration_ms: duration_ms(phase_started_at.elapsed()),
            rss_bytes: memory.rss_bytes,
            peak_rss_bytes: memory.peak_rss_bytes,
        });
        result
    }

    pub(crate) fn finish(self) -> PatchMetrics {
        let memory = sample_memory();
        PatchMetrics {
            total_duration_ms: duration_ms(self.started_at.elapsed()),
            final_rss_bytes: memory.rss_bytes,
            peak_rss_bytes: memory.peak_rss_bytes,
            phases: self.phases,
        }
    }
}

fn duration_ms(duration: std::time::Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn sample_memory() -> MemorySample {
    #[cfg(target_os = "linux")]
    {
        sample_linux_memory().unwrap_or_else(sample_unix_peak_memory)
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        sample_unix_peak_memory()
    }

    #[cfg(not(unix))]
    {
        MemorySample::default()
    }
}

#[cfg(target_os = "linux")]
fn sample_linux_memory() -> Option<MemorySample> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let mut sample = MemorySample::default();

    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            sample.rss_bytes = parse_kib_line(rest);
        } else if let Some(rest) = line.strip_prefix("VmHWM:") {
            sample.peak_rss_bytes = parse_kib_line(rest);
        }
    }

    if sample.rss_bytes.is_none() && sample.peak_rss_bytes.is_none() {
        None
    } else {
        Some(sample)
    }
}

#[cfg(target_os = "linux")]
fn parse_kib_line(value: &str) -> Option<u64> {
    let kib = value.split_whitespace().next()?.parse::<u64>().ok()?;
    kib.checked_mul(1024)
}

#[cfg(unix)]
fn sample_unix_peak_memory() -> MemorySample {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc != 0 {
        return MemorySample::default();
    }

    let usage = unsafe { usage.assume_init() };
    #[cfg(target_os = "macos")]
    let peak_rss_bytes = u64::try_from(usage.ru_maxrss).ok();
    #[cfg(not(target_os = "macos"))]
    let peak_rss_bytes = u64::try_from(usage.ru_maxrss)
        .ok()
        .and_then(|kib| kib.checked_mul(1024));

    MemorySample {
        rss_bytes: None,
        peak_rss_bytes,
    }
}
