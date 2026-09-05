// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{ensure, Context, Result};
use reseam_apk::ApkFile;
use reseam_patcher::context::PatchContext;
use reseam_patcher::engine::{self, PatchResult, PatchStatus};
use reseam_patcher::Patch;

use crate::dto::{PatchOutcome, PatchOutput, PatchRequest, RunEvent};
use crate::inspect::{load_bundles, open_apk};
use crate::metrics::{ApplyDiagnostics, PatchPhase, PatchProfiler};
use crate::output::write_signed;

/// Runs the request end to end: open, load, apply, write, sign. A dry run
/// stops after validation and reports what would run.
pub fn patch(request: &PatchRequest, mut emit: impl FnMut(RunEvent)) -> Result<PatchOutcome> {
    let mut profiler = PatchProfiler::new();
    let results = run(request, &mut emit, &mut profiler)?;
    Ok(PatchOutcome {
        results,
        metrics: profiler.finish(),
    })
}

fn run(
    request: &PatchRequest,
    emit: &mut impl FnMut(RunEvent),
    profiler: &mut PatchProfiler,
) -> Result<Vec<PatchResult>> {
    emit(info(format!("Opening APK {}", request.apk_path.display())));
    let mut apk = profiler.measure(PatchPhase::OpenApk, || {
        open_apk(
            &request.apk_path,
            &request.split_paths,
            &ApkFile::patch_options(),
        )
    })?;
    if let PatchOutput::SingleFile { .. } = request.output {
        ensure!(
            apk.components().len() == 1,
            "single-file output needs an APK without splits; use a split directory"
        );
    }

    emit(info("Loading bundles".to_string()));
    let bundles = profiler.measure(PatchPhase::LoadBundles, || {
        load_bundles(&request.bundle_paths, &request.trust)
    })?;
    let patches: Vec<&dyn Patch> = bundles
        .iter()
        .flat_map(|bundle| bundle.patches.iter().map(Box::as_ref))
        .collect();

    if request.dry_run {
        let results = profiler.measure(PatchPhase::ValidatePatches, || {
            engine::validate_patches(
                &patches,
                &request.selection,
                apk.package_name().as_deref(),
                apk.version_name().as_deref(),
            )
        })?;
        for result in &results {
            emit(RunEvent::PatchFinished {
                patch: result.name.clone(),
                status: result.status.clone(),
            });
        }
        ensure_none_failed(&results)?;
        return Ok(results);
    }

    let mut ctx = PatchContext::new(&mut apk);
    let results = profiler
        .measure(PatchPhase::ApplyPatches, || {
            engine::apply_patches(&mut ctx, &patches, &request.selection, |event| {
                emit(event.into())
            })
        })
        .context("patch application failed")?;
    profiler.set_apply_diagnostics(apply_diagnostics(&ctx));
    drop(ctx);

    ensure_none_failed(&results)?;

    emit(info(format!(
        "Writing signed output to {}",
        request.output.path().display()
    )));
    write_signed(apk, &request.output, request.signing.as_ref(), profiler)?;
    drop(bundles);
    release_process_memory();
    Ok(results)
}

fn info(message: String) -> RunEvent {
    RunEvent::Info { message }
}

fn ensure_none_failed(results: &[PatchResult]) -> Result<()> {
    let failed: Vec<&str> = results
        .iter()
        .filter(|result| matches!(result.status, PatchStatus::Failed { .. }))
        .map(|result| result.name.as_str())
        .collect();
    ensure!(
        failed.is_empty(),
        "{} patch(es) failed: {}",
        failed.len(),
        failed.join(", ")
    );
    Ok(())
}

/// Sampled right after `apply_patches`, at the apply-phase memory peak, to
/// attribute RSS to materialized DEX IR vs the in-process JVM vs everything else.
fn apply_diagnostics(ctx: &PatchContext) -> ApplyDiagnostics {
    ApplyDiagnostics {
        rss_bytes: PatchProfiler::current_rss_bytes(),
        dex: ctx.apk().dex().memory_breakdown(),
        jvm: reseam_patcher::jvm_heap_stats(),
    }
}

/// Hands run-scoped memory back to the system so a long-lived host does not
/// carry one run's peak into the next: the runtime's garbage first, then the
/// native allocator's retained pages.
fn release_process_memory() {
    reseam_patcher::release_runtime_memory();
    purge_native_heap();
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn purge_native_heap() {
    // SAFETY: malloc_trim only releases free memory held by the allocator.
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(target_os = "android")]
fn purge_native_heap() {
    const M_PURGE: libc::c_int = -101;
    extern "C" {
        fn mallopt(param: libc::c_int, value: libc::c_int) -> libc::c_int;
    }
    // SAFETY: M_PURGE asks scudo to release cached free pages; it touches no live allocation.
    unsafe {
        mallopt(M_PURGE, 0);
    }
}

#[cfg(not(any(all(target_os = "linux", target_env = "gnu"), target_os = "android")))]
fn purge_native_heap() {}
