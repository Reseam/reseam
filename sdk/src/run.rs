// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::Result;
use reseam_apk::ApkFile;
use reseam_patcher::bundle::PatchBundle;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use reseam_patcher::engine::{self, AppliedRun, Delivery, PatchResult, PatchStatus, ResolvedPlan};
use reseam_patcher::{ApkVersion, Patch, PatchSpec};

use crate::TrustStore;
use crate::error::Problem;
use crate::inspect::{OpenedApk, PreparedInspection, TrustedArchive, open_apk, open_bundles};
use crate::memory::release_process_memory;
use crate::metrics::{ApplyDiagnostics, PatchPhase, PatchProfiler};
use crate::output::{resolve_output, stage_unsigned};
use crate::{InstallMethod, PatchArtifact, PatchOutcome, PatchRequest, RunEvent};
use std::borrow::Cow;
use std::collections::HashSet;
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
    let output = resolve_output(&request.output, opened.apk.components().len())?;

    emit(info("Loading bundles".to_string()));
    let archives = match prepared {
        Some(prepared) => prepared.trusted_archives(trust)?,
        None => open_bundles(
            &request
                .bundle_paths
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>(),
            trust,
        )?,
    };
    let package = opened.apk.package_name().map(Cow::into_owned);
    let version_name = opened.apk.version_name().map(Cow::into_owned);
    let version = ApkVersion {
        name: version_name.as_deref(),
        code: opened.apk.version_code(),
    };
    let specs: Vec<&PatchSpec> = archives.iter().flat_map(TrustedArchive::catalog).collect();
    let selection = resolve(&specs, package.as_deref())?;
    let plan = ResolvedPlan::resolve(
        &specs,
        &selection,
        delivery(request.install_method),
        package.as_deref(),
        version,
    )?;

    if request.dry_run {
        let results = profiler.measure(PatchPhase::ValidatePatches, || {
            Ok::<_, crate::HostError>(engine::validate_patches(&specs, &plan))
        })?;
        for result in &results {
            emit(RunEvent::PatchFinished {
                patch: result.patch.clone(),
                status: result.status.clone(),
            });
        }
        if !request.leave_out_failed {
            ensure_none_failed(&results)?;
        }
        return Ok((results, output));
    }

    let executed = executed_patches(&archives, &plan);
    let bundles = profiler.measure(PatchPhase::LoadBundles, || {
        archives
            .into_iter()
            .zip(&executed)
            .map(|(archive, patches)| archive.load(patches))
            .collect::<Result<Vec<_>>>()
    })?;
    let mut applied = apply(&mut opened, &bundles, plan, emit, profiler)?;
    if !request.leave_out_failed {
        ensure_none_failed(applied.results())?;
    }

    emit(info(format!(
        "Writing signed output to {}",
        output.path().display()
    )));
    // Patching is over, so its search indexes and resident source pages go
    // before writing, the run's peak.
    for index in 0..opened.apk.dex().len() {
        let added = opened.apk.is_added_dex(index);
        let dex = opened
            .apk
            .dex_mut(index)
            .expect("index is within the DEX container");
        dex.release_search_caches();
        dex.release_pages();
        if !added && dex.is_dirty() {
            dex.set_write_options(reseam_apk::reseam_dex::write::WriteOptions {
                debug_info: reseam_apk::reseam_dex::write::MetadataPolicy::OmitOriginal,
                link_data: reseam_apk::reseam_dex::write::LinkPolicy::Omit,
                ..Default::default()
            });
        }
    }
    release_process_memory();
    let staged = stage_unsigned(opened.apk, &output, request.signing.as_ref(), profiler)?;
    if applied.rewrites_output() {
        // The patch model is gone; give its memory back before patch code works
        // on whole APK files.
        release_process_memory();
        profiler.measure(PatchPhase::RewriteOutput, || {
            applied.rewrite_output(&mut staged.written(), &mut *emit)
        })?;
    }
    staged.sign_and_publish(profiler)?;
    Ok((applied.into_results(), output))
}

/// The ids of the patches `plan` executes, per archive. Plan positions follow
/// the archives' catalogs concatenated in order.
fn executed_patches(archives: &[TrustedArchive], plan: &ResolvedPlan) -> Vec<HashSet<String>> {
    let mut positions = 0..;
    archives
        .iter()
        .map(|archive| {
            archive
                .catalog()
                .iter()
                .zip(positions.by_ref())
                .filter(|&(_, idx)| plan.executes(idx))
                .map(|(spec, _)| spec.id.clone())
                .collect()
        })
        .collect()
}

fn delivery(method: InstallMethod) -> Delivery {
    match method {
        InstallMethod::Install => Delivery::Install,
        InstallMethod::Mount => Delivery::Mount,
    }
}

/// Applies `plan` to `opened`.
fn apply<'b>(
    opened: &mut OpenedApk,
    bundles: &'b [PatchBundle],
    plan: ResolvedPlan,
    emit: &mut impl FnMut(RunEvent),
    profiler: &mut PatchProfiler,
) -> Result<AppliedRun<'b>> {
    let specs: Vec<&PatchSpec> = bundles.iter().flat_map(PatchBundle::catalog).collect();
    let patches: Vec<&Patch> = bundles.iter().flat_map(PatchBundle::patches).collect();
    let extension_paths: Vec<_> = bundles
        .iter()
        .flat_map(|bundle| bundle.extension_dex().iter().cloned())
        .collect();
    let mut ctx = PatchContext::new(&mut opened.apk);
    ctx.set_extensions(ExtensionSet::load(&extension_paths)?);
    let applied = profiler.measure(PatchPhase::ApplyPatches, || {
        engine::apply_patches(&mut ctx, &specs, plan, &patches, &mut *emit)
    })?;
    profiler.set_apply_diagnostics(apply_diagnostics(&ctx));
    Ok(applied)
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
