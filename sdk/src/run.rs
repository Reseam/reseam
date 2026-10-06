// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use reseam_apk::ApkFile;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use reseam_patcher::engine::{self, Delivery, PatchResult, PatchStatus};
use reseam_patcher::{Patch, PatchSpec};

use crate::TrustStore;
use crate::error::Problem;
use crate::inspect::{OpenedApk, PreparedInspection, load_bundles, open_apk};
use crate::metrics::{ApplyDiagnostics, PatchPhase, PatchProfiler};
use crate::output::write_signed;
use crate::{InstallMethod, PatchArtifact, PatchOutcome, PatchRequest, RunEvent};
use std::path::{Path, PathBuf};

/// Runs the request end to end: open, load, apply, write, sign. A dry run
/// stops after validation and reports what would run without publishing artifacts.
///
/// Progress runs synchronously on the calling thread; callbacks must not re-enter
/// the engine. Output destinations must be distinct and must not overwrite the
/// signing identity. Completed artifacts are staged beside their destinations.
/// Signing and publication failures preserve previous outputs; if filesystem
/// errors also prevent rollback, the error identifies the retained recovery directory.
pub fn patch(request: &PatchRequest, emit: impl FnMut(RunEvent)) -> Result<PatchOutcome> {
    patch_with_selection(request, |_, _| Ok(request.selection.clone()), emit)
}

/// Resolves host selection syntax against the specifications and package already
/// loaded by this run, then validates or applies the returned selection.
///
/// The resolver runs once after trusted bundle loading. Its specifications are in
/// execution-list order and valid only during the call. Errors stop the run before
/// editing or publication. Shares [patch]'s output and callback contracts.
pub fn patch_with_selection(
    request: &PatchRequest,
    resolve: impl FnOnce(&[&PatchSpec], Option<&str>) -> Result<reseam_model::PatchSelection>,
    emit: impl FnMut(RunEvent),
) -> Result<PatchOutcome> {
    patch_with_inputs(request, None, resolve, emit)
}

pub(crate) fn patch_with_inputs(
    request: &PatchRequest,
    prepared: Option<PreparedInspection>,
    resolve: impl FnOnce(&[&PatchSpec], Option<&str>) -> Result<reseam_model::PatchSelection>,
    mut emit: impl FnMut(RunEvent),
) -> Result<PatchOutcome> {
    let trust = TrustStore::from_hex(&request.trust.keys)?;
    let mut profiler = PatchProfiler::new();
    let result = run(request, &trust, prepared, resolve, &mut emit, &mut profiler);
    release_process_memory();
    let (results, output) = result?;
    Ok(PatchOutcome {
        output,
        results,
        metrics: profiler.finish(),
    })
}

fn run(
    request: &PatchRequest,
    trust: &TrustStore,
    mut prepared: Option<PreparedInspection>,
    resolve: impl FnOnce(&[&PatchSpec], Option<&str>) -> Result<reseam_model::PatchSelection>,
    emit: &mut impl FnMut(RunEvent),
    profiler: &mut PatchProfiler,
) -> Result<(Vec<PatchResult>, PatchArtifact)> {
    emit(info(format!("Opening APK {}", request.apk_path)));
    let mut opened = profiler.measure(PatchPhase::OpenApk, || {
        if let Some(prepared) = prepared.as_mut() {
            return prepared
                .opened
                .take()
                .ok_or(crate::HostError::InvalidRequest(
                    "prepared inspection has no APK",
                ));
        }
        open_input(request)
    })?;
    report_container(&opened, emit);
    let output = request.output.resolve(opened.apk.components().len())?;

    emit(info("Loading bundles".to_string()));
    let bundles = profiler.measure(PatchPhase::LoadBundles, || {
        if let Some(prepared) = prepared {
            return prepared.load_bundles(trust);
        }
        load_bundles(
            &request
                .bundle_paths
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>(),
            trust,
        )
    })?;
    let patches: Vec<&Patch> = bundles
        .iter()
        .flat_map(|bundle| bundle.patches().iter())
        .collect();

    let specs: Vec<_> = patches.iter().map(|patch| patch.spec()).collect();
    let selection = resolve(&specs, opened.apk.package_name().as_deref())?;

    if request.dry_run {
        let results = profiler.measure(PatchPhase::ValidatePatches, || {
            engine::validate_patches(
                &patches,
                &selection,
                opened.apk.package_name().as_deref(),
                opened.apk.version_name().as_deref(),
            )
        })?;
        for result in &results {
            emit(RunEvent::PatchFinished {
                patch: result.patch.clone(),
                status: result.status.clone(),
            });
        }
        ensure_none_failed(&results)?;
        return Ok((results, output));
    }

    let extension_paths: Vec<_> = bundles
        .iter()
        .flat_map(|bundle| bundle.extension_dex().iter().cloned())
        .collect();
    let results = apply(
        request,
        &mut opened,
        &patches,
        &selection,
        &extension_paths,
        emit,
        profiler,
    )?;

    ensure_none_failed(&results)?;

    emit(info(format!(
        "Writing signed output to {}",
        output.path().display()
    )));
    for index in 0..opened.apk.dex().len() {
        if opened.apk.is_added_dex(index) {
            continue;
        }
        let dex = opened
            .apk
            .dex_mut(index)
            .expect("index is within the DEX container");
        if dex.is_dirty() {
            dex.set_write_options(reseam_apk::reseam_dex::write::WriteOptions {
                debug_info: reseam_apk::reseam_dex::write::MetadataPolicy::OmitOriginal,
                link_data: reseam_apk::reseam_dex::write::LinkPolicy::Omit,
                ..Default::default()
            });
        }
    }
    write_signed(opened.apk, &output, request.signing.as_ref(), profiler)?;
    Ok((results, output))
}

/// Applies the selection to `opened`. A mount build that finds patches editing
/// the manifest reopens the original input into `opened` and applies again
/// without them, since the APK still holds their edits.
fn apply(
    request: &PatchRequest,
    opened: &mut OpenedApk,
    patches: &[&Patch],
    selection: &reseam_model::PatchSelection,
    extension_paths: &[PathBuf],
    emit: &mut impl FnMut(RunEvent),
    profiler: &mut PatchProfiler,
) -> Result<Vec<PatchResult>> {
    let mut unmountable = Vec::new();
    loop {
        let delivery = match request.install_method {
            InstallMethod::Install => Delivery::Install,
            InstallMethod::Mount => Delivery::Mount {
                unmountable: &unmountable,
            },
        };
        let mut ctx = PatchContext::new(&mut opened.apk);
        ctx.set_extensions(ExtensionSet::load(extension_paths)?);
        let results = profiler.measure(PatchPhase::ApplyPatches, || {
            engine::apply_patches(&mut ctx, patches, selection, delivery, |event| {
                emit(event.into());
            })
        })?;
        profiler.set_apply_diagnostics(apply_diagnostics(&ctx));
        drop(ctx);

        let found: Vec<String> = results
            .iter()
            .filter(|result| matches!(result.status, PatchStatus::Unmountable { .. }))
            .map(|result| result.patch.clone())
            .filter(|patch| !unmountable.contains(patch))
            .collect();
        if found.is_empty() {
            return Ok(results);
        }
        unmountable.extend(found);
        emit(RunEvent::Restarted {
            unmountable: unmountable.clone(),
        });
        *opened = profiler.measure(PatchPhase::OpenApk, || open_input(request))?;
    }
}

fn open_input(request: &PatchRequest) -> Result<OpenedApk> {
    open_apk(
        Path::new(&request.apk_path),
        &request
            .split_paths
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>(),
        ApkFile::patch_options(),
    )
}

fn report_container(opened: &OpenedApk, emit: &mut impl FnMut(RunEvent)) {
    if let Some(bundle) = &opened.bundle {
        let splits = opened.apk.components().len() - 1;
        emit(info(format!(
            "Opened {} bundle {}: {} base APK, {} split{}",
            bundle.as_str(),
            opened.apk.package_name().as_deref().unwrap_or_default(),
            opened
                .apk
                .base()
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            splits,
            if splits == 1 { "" } else { "s" },
        )));
    }
}

fn info(message: String) -> RunEvent {
    RunEvent::Info { message }
}

fn ensure_none_failed(results: &[PatchResult]) -> Result<()> {
    let patches: Vec<_> = results
        .iter()
        .filter(|result| matches!(result.status, PatchStatus::Failed { .. }))
        .map(|result| result.patch.clone())
        .collect();
    if !patches.is_empty() {
        return Err(Problem::PatchesFailed { patches }.into());
    }
    Ok(())
}

fn apply_diagnostics(ctx: &PatchContext<'_>) -> ApplyDiagnostics {
    let dex = ctx.apk().dex().memory_breakdown();
    ApplyDiagnostics {
        rss_bytes: PatchProfiler::current_rss_bytes(),
        dex: reseam_model::MemoryBreakdown {
            raw_buffer_bytes: dex.raw_buffer_bytes,
            string_pool_bytes: dex.string_pool_bytes,
            string_count: dex.string_count,
            id_table_bytes: dex.id_table_bytes,
            class_def_bytes: dex.class_def_bytes,
            materialized: reseam_model::MaterializationStats {
                total_classes: dex.materialized.total_classes,
                resolved_classes: dex.materialized.resolved_classes,
                methods: dex.materialized.methods,
                instructions: dex.materialized.instructions,
            },
        },
        jvm: reseam_patcher::jvm_heap_stats(),
    }
}

// Current RSS can fall after collection and cache purging; process high-water
// counters deliberately remain unchanged.
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
    unsafe extern "C" {
        fn mallopt(param: libc::c_int, value: libc::c_int) -> libc::c_int;
    }
    // SAFETY: M_PURGE asks scudo to release cached free pages; it touches no live allocation.
    unsafe {
        mallopt(M_PURGE, 0);
    }
}

#[cfg(not(any(all(target_os = "linux", target_env = "gnu"), target_os = "android")))]
fn purge_native_heap() {}
