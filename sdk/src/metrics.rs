// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use reseam_apk::reseam_dex::MemoryBreakdown;
use reseam_patcher::JvmHeapStats;
use serde::{Deserialize, Serialize};

static HEAP_LIVE: AtomicUsize = AtomicUsize::new(0);
static HEAP_PEAK: AtomicUsize = AtomicUsize::new(0);
static TRACE_STEP: AtomicUsize = AtomicUsize::new(0);
static TRACE_NEXT: AtomicUsize = AtomicUsize::new(usize::MAX);

thread_local! {
    static IN_TRACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Prints a backtrace every time the live heap grows by another `step`
/// bytes past its previous high-water mark, which attributes a peak to the
/// allocations that built it. Driven by `RESEAM_HEAP_TRACE=<MiB>`.
pub fn trace_heap_growth(step: usize) {
    TRACE_STEP.store(step, Ordering::Relaxed);
    TRACE_NEXT.store(HEAP_LIVE.load(Ordering::Relaxed) + step, Ordering::Relaxed);
}

fn maybe_trace(live: usize) {
    if live < TRACE_NEXT.load(Ordering::Relaxed) {
        return;
    }
    IN_TRACE.with(|flag| {
        if flag.replace(true) {
            return;
        }
        let step = TRACE_STEP.load(Ordering::Relaxed);
        TRACE_NEXT.store(live + step, Ordering::Relaxed);
        eprintln!(
            "heap trace: live {} MiB\n{}",
            live >> 20,
            std::backtrace::Backtrace::force_capture()
        );
        flag.set(false);
    });
}

/// Counts bytes allocated through the global allocator so live and peak heap
/// can be read without asking the allocator, which glibc, scudo and jemalloc
/// each answer differently. Install with `#[global_allocator]`.
pub struct CountingAllocator;

impl CountingAllocator {
    fn add(bytes: usize) {
        let live = HEAP_LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
        HEAP_PEAK.fetch_max(live, Ordering::Relaxed);
        maybe_trace(live);
    }

    fn remove(bytes: usize) {
        HEAP_LIVE.fetch_sub(bytes, Ordering::Relaxed);
    }
}

// SAFETY: every method forwards to `System` unchanged; only the counters are
// added, and they never influence the returned pointers.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            Self::add(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() {
            Self::add(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        Self::remove(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            Self::remove(layout.size());
            Self::add(new_size);
        }
        new_ptr
    }
}

fn heap_live_bytes() -> Option<u64> {
    let live = HEAP_LIVE.load(Ordering::Relaxed);
    (live > 0).then_some(live as u64)
}

/// Highest live heap since the last [`reset_heap_peak`]; `None` when no
/// [`CountingAllocator`] is installed.
fn heap_peak_bytes() -> Option<u64> {
    let peak = HEAP_PEAK.load(Ordering::Relaxed);
    (peak > 0).then_some(peak as u64)
}

fn reset_heap_peak() {
    HEAP_PEAK.store(HEAP_LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchPhase {
    OpenApk,
    LoadBundles,
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
    pub heap_live_bytes: Option<u64>,
    /// Highest live heap during the phase, independent of what the allocator
    /// keeps cached afterwards.
    pub heap_peak_bytes: Option<u64>,
}

/// Sampled right after `apply_patches`, at the apply-phase memory peak, to
/// attribute RSS to materialized DEX IR vs the in-process JVM vs everything else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyDiagnostics {
    pub rss_bytes: Option<u64>,
    pub dex: MemoryBreakdown,
    pub jvm: Option<JvmHeapStats>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatchMetrics {
    pub total_duration_ms: u64,
    pub final_rss_bytes: Option<u64>,
    pub peak_rss_bytes: Option<u64>,
    pub final_heap_live_bytes: Option<u64>,
    pub final_rss_anon_bytes: Option<u64>,
    pub final_rss_file_bytes: Option<u64>,
    pub phases: Vec<PatchPhaseMetrics>,
    pub apply_diagnostics: Option<ApplyDiagnostics>,
}

#[derive(Debug, Clone, Copy, Default)]
struct MemorySample {
    rss_bytes: Option<u64>,
    peak_rss_bytes: Option<u64>,
    heap_live_bytes: Option<u64>,
    rss_anon_bytes: Option<u64>,
    rss_file_bytes: Option<u64>,
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
        sample_memory().rss_bytes
    }

    pub(crate) fn measure<T, E>(
        &mut self,
        phase: PatchPhase,
        run: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let phase_started_at = Instant::now();
        reset_heap_peak();
        let result = run();
        let memory = sample_memory();
        self.phases.push(PatchPhaseMetrics {
            phase,
            duration_ms: duration_ms(phase_started_at.elapsed()),
            rss_bytes: memory.rss_bytes,
            peak_rss_bytes: memory.peak_rss_bytes,
            heap_live_bytes: memory.heap_live_bytes,
            heap_peak_bytes: heap_peak_bytes(),
        });
        result
    }

    pub(crate) fn finish(self) -> PatchMetrics {
        let memory = sample_memory();
        PatchMetrics {
            total_duration_ms: duration_ms(self.started_at.elapsed()),
            final_rss_bytes: memory.rss_bytes,
            peak_rss_bytes: memory.peak_rss_bytes,
            final_heap_live_bytes: memory.heap_live_bytes,
            final_rss_anon_bytes: memory.rss_anon_bytes,
            final_rss_file_bytes: memory.rss_file_bytes,
            phases: self.phases,
            apply_diagnostics: self.apply_diagnostics,
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
    let mut sample = MemorySample {
        heap_live_bytes: heap_live_bytes(),
        ..MemorySample::default()
    };

    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            sample.rss_bytes = parse_kib_line(rest);
        } else if let Some(rest) = line.strip_prefix("VmHWM:") {
            sample.peak_rss_bytes = parse_kib_line(rest);
        } else if let Some(rest) = line.strip_prefix("RssAnon:") {
            sample.rss_anon_bytes = parse_kib_line(rest);
        } else if let Some(rest) = line.strip_prefix("RssFile:") {
            sample.rss_file_bytes = parse_kib_line(rest);
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
        heap_live_bytes: heap_live_bytes(),
        rss_bytes: None,
        peak_rss_bytes,
        rss_anon_bytes: None,
        rss_file_bytes: None,
    }
}
