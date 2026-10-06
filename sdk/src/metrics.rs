// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::{Duration, Instant};

pub use reseam_model::{ApplyDiagnostics, PatchMetrics, PatchPhase, PatchPhaseMetrics};

#[derive(Default)]
struct MemorySample {
    rss: Option<u64>,
    process_peak: Option<u64>,
    anonymous: Option<u64>,
    file_backed: Option<u64>,
}

pub(crate) struct PatchProfiler {
    started_at: Instant,
    phases: Vec<PatchPhaseMetrics>,
    apply_diagnostics: Option<ApplyDiagnostics>,
}

impl PatchProfiler {
    pub(crate) fn new() -> Self {
        Self {
            started_at: Instant::now(),
            phases: Vec::new(),
            apply_diagnostics: None,
        }
    }

    pub(crate) fn set_apply_diagnostics(&mut self, diagnostics: ApplyDiagnostics) {
        self.apply_diagnostics = Some(diagnostics);
    }

    pub(crate) fn current_rss_bytes() -> Option<u64> {
        sample_memory().rss
    }

    pub(crate) fn measure<T, E>(
        &mut self,
        phase: PatchPhase,
        run: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let started_at = Instant::now();
        let result = run();
        let elapsed = started_at.elapsed();
        let memory = sample_memory();
        self.phases.push(PatchPhaseMetrics {
            phase,
            duration_ms: duration_ms(elapsed),
            rss_bytes: memory.rss,
            // The wire field is retained for hosts; this is the process high-water
            // mark observed at phase end, not an interval peak.
            peak_rss_bytes: memory.process_peak,
            heap_live_bytes: None,
            heap_peak_bytes: None,
        });
        result
    }

    pub(crate) fn finish(self) -> PatchMetrics {
        let memory = sample_memory();
        PatchMetrics {
            total_duration_ms: duration_ms(self.started_at.elapsed()),
            final_rss_bytes: memory.rss,
            peak_rss_bytes: memory.process_peak,
            final_heap_live_bytes: None,
            final_rss_anon_bytes: memory.anonymous,
            final_rss_file_bytes: memory.file_backed,
            phases: self.phases,
            apply_diagnostics: self.apply_diagnostics,
        }
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn sample_memory() -> MemorySample {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return sample_unix_peak_memory();
    };
    let field = |name| {
        status.lines().find_map(|line| {
            line.strip_prefix(name)?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()?
                .checked_mul(1024)
        })
    };
    MemorySample {
        rss: field("VmRSS:"),
        process_peak: field("VmHWM:"),
        anonymous: field("RssAnon:"),
        file_backed: field("RssFile:"),
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn sample_memory() -> MemorySample {
    sample_unix_peak_memory()
}

#[cfg(unix)]
fn sample_unix_peak_memory() -> MemorySample {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: usage is writable storage for getrusage; it is read only on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return MemorySample::default();
    }
    // SAFETY: successful getrusage initialized the structure.
    let usage = unsafe { usage.assume_init() };
    let process_peak = u64::try_from(usage.ru_maxrss).ok();
    #[cfg(not(target_os = "macos"))]
    let process_peak = process_peak.and_then(|kib| kib.checked_mul(1024));
    MemorySample {
        process_peak,
        ..MemorySample::default()
    }
}

#[cfg(windows)]
fn sample_memory() -> MemorySample {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut counters = std::mem::MaybeUninit::<PROCESS_MEMORY_COUNTERS>::uninit();
    // SAFETY: the current-process pseudo-handle is valid; counters is writable
    // storage of the declared size and is read only after a successful call.
    if unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            counters.as_mut_ptr(),
            size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    } == 0
    {
        return MemorySample::default();
    }
    // SAFETY: successful GetProcessMemoryInfo initialized the structure.
    let counters = unsafe { counters.assume_init() };
    MemorySample {
        rss: Some(counters.WorkingSetSize as u64),
        process_peak: Some(counters.PeakWorkingSetSize as u64),
        ..MemorySample::default()
    }
}

#[cfg(not(any(unix, windows)))]
fn sample_memory() -> MemorySample {
    MemorySample::default()
}
