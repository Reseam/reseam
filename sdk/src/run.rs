// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};

use reseam_dex::{LinkPolicy, MetadataPolicy, WriteOptions};
use reseam_model::{
    Abi, AppIdentity, BuildRecord, PatchArtifact, PatchOutcome, PatchPhase, PatchRef, PatchRequest,
    Problem, RecordedPatch, RunEvent,
};
use reseam_patcher::PatchSpec;
use reseam_patcher::engine::{self, PatchResult, PatchStatus, PlanOptions, ResolvedPlan, Trace};

use crate::apk::OpenedApk;
use crate::apply::{apply, load_executed};
use crate::bundles::{OpenedBundle, TrustedArchive, trusted_archives};
use crate::error::Result;
use crate::inputs::Inputs;
use crate::memory::release_process_memory;
use crate::output::{resolve_output, stage_unsigned};
use crate::progress::RunProgress;
use crate::trust::TrustStore;

/// Runs the request end to end: open, load, apply, write, sign. A dry run
/// stops after validation and reports what would run without publishing artifacts.
///
/// Progress runs synchronously on the calling thread; callbacks must not re-enter
/// the engine. Output destinations must be distinct and must not overwrite the
/// signing identity. Completed artifacts are staged beside their destinations.
/// Signing and publication failures preserve previous outputs; if filesystem
/// errors also prevent rollback, the error identifies the retained recovery directory.
pub fn patch(request: &PatchRequest, emit: impl FnMut(RunEvent)) -> Result<PatchOutcome> {
    let inputs = Inputs::open(
        Some(&request.apk_path),
        &request.split_paths,
        &request.device_abis,
        &request.bundle_paths,
    )?;
    run(request, inputs, emit)
}

/// Runs `request` on the inputs opened for it.
pub(crate) fn run(
    request: &PatchRequest,
    inputs: Inputs,
    mut emit: impl FnMut(RunEvent),
) -> Result<PatchOutcome> {
    let trust = TrustStore::new(&request.trust)?;
    let Inputs {
        apk,
        opening,
        bundles,
    } = inputs;
    let opened = apk.expect("patch inputs are opened with the request's APK");
    let mut progress = RunProgress::new(opening, &mut emit);
    let result = execute(request, &trust, opened, bundles, &mut progress);
    release_process_memory();
    let Built {
        results,
        output,
        record,
        native_abi,
    } = result?;
    Ok(PatchOutcome {
        output,
        results,
        record,
        metrics: progress.finish(),
        native_abi,
    })
}

/// What a run made, before its metrics are complete.
struct Built {
    results: Vec<PatchResult>,
    output: PatchArtifact,
    record: BuildRecord,
    native_abi: Option<Abi>,
}

fn execute(
    request: &PatchRequest,
    trust: &TrustStore,
    mut opened: OpenedApk,
    bundles: Vec<OpenedBundle>,
    progress: &mut RunProgress<'_>,
) -> Result<Built> {
    report_container(&opened, progress);
    let output = resolve_output(&request.output, opened.apk.components().len())?;

    let archives = trusted_archives(bundles, trust)?;
    let identity = opened.identity();
    let specs: Vec<&PatchSpec> = archives.iter().flat_map(TrustedArchive::catalog).collect();
    let package = identity.package_name.as_deref();
    let plan = ResolvedPlan::resolve(
        &specs,
        &request.selection,
        request.install_method.into(),
        package,
        identity.version(),
    )?;
    let options = PlanOptions::resolve(&specs, &request.selection, &plan, package)?;
    let disabled: HashSet<PatchRef> = specs
        .iter()
        .enumerate()
        .filter(|&(idx, _)| plan.is_disabled(idx))
        .map(|(_, spec)| spec.reference())
        .collect();
    let fingerprints: HashMap<PatchRef, String> = archives
        .iter()
        .flat_map(TrustedArchive::fingerprints)
        .map(|(patch, fingerprint)| (patch, fingerprint.to_owned()))
        .collect();

    if request.dry_run {
        let results = progress.measure(PatchPhase::ValidatePatches, |_| {
            Ok::<_, crate::HostError>(engine::validate_patches(&specs, &plan))
        })?;
        for result in &results {
            progress.emit(RunEvent::PatchFinished {
                patch: result.patch.clone(),
                status: result.status.clone(),
            });
        }
        if !request.leave_out_failed {
            ensure_none_failed(&results)?;
        }
        let record = record(request, identity, &results, &disabled, &fingerprints);
        return Ok(Built {
            results,
            output,
            record,
            native_abi: None,
        });
    }

    let bundles = load_executed(archives, &plan, progress)?;
    let trace = if request.trace {
        Trace::Log
    } else {
        Trace::Off
    };
    let mut applied = apply(&mut opened, &bundles, plan, &options, trace, progress)?;
    if !request.leave_out_failed {
        ensure_none_failed(applied.results())?;
    }
    // Patches can add native libraries, so the ABI is chosen from the patched app.
    let native_abi = opened.retain_native_abi_for(&request.device_abis)?;

    // Patching is over, so its search indexes and resident source pages go
    // before writing, the run's peak.
    opened.apk.prepare_dex_for_writing(WriteOptions {
        debug_info: MetadataPolicy::OmitOriginal,
        link_data: LinkPolicy::Omit,
        ..Default::default()
    });
    release_process_memory();
    let staged = stage_unsigned(opened.apk, &output, request.signing.as_ref(), progress)?;
    if applied.rewrites_output() {
        // The patch model is gone; give its memory back before patch code works
        // on whole APK files.
        release_process_memory();
        progress.measure(PatchPhase::RewriteOutput, |progress| {
            applied.rewrite_output(&mut staged.written(), |event| progress.emit(event))
        })?;
    }
    staged.sign_and_publish(progress)?;
    let results = applied.into_results();
    let record = record(request, identity, &results, &disabled, &fingerprints);
    Ok(Built {
        results,
        output,
        record,
        native_abi,
    })
}

/// What the run of `request` on `app` built: its selection and install
/// method, and the fingerprint of each patch in `results` the run took up.
/// A patch the selection turns off is reported only when a selected patch
/// needs it, and was never taken up.
fn record(
    request: &PatchRequest,
    app: AppIdentity,
    results: &[PatchResult],
    disabled: &HashSet<PatchRef>,
    fingerprints: &HashMap<PatchRef, String>,
) -> BuildRecord {
    BuildRecord {
        app,
        selection: request.selection.clone(),
        install_method: request.install_method,
        patches: results
            .iter()
            .filter(|result| !disabled.contains(&result.patch))
            .map(|result| RecordedPatch {
                patch: result.patch.clone(),
                fingerprint: fingerprints[&result.patch].clone(),
            })
            .collect(),
    }
}

fn report_container(opened: &OpenedApk, progress: &mut RunProgress<'_>) {
    if let Some(bundle) = &opened.bundle {
        let splits = opened.apk.components().len() - 1;
        progress.emit(RunEvent::Info {
            message: format!(
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
            ),
        });
    }
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
