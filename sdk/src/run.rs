// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use reseam_model::{
    InstallMethod, PatchArtifact, PatchOutcome, PatchPhase, PatchRequest, Problem, RunEvent,
};
use reseam_patcher::engine::{self, Delivery, PatchResult, PatchStatus, ResolvedPlan};
use reseam_patcher::{ApkVersion, PatchSpec};

use crate::apk::OpenedApk;
use crate::apply::{apply, load_executed, release_for_writing};
use crate::bundles::{OpenedBundle, TrustedArchive, trusted_archives};
use crate::error::Result;
use crate::inputs::Inputs;
use crate::memory::release_process_memory;
use crate::metrics::PatchProfiler;
use crate::output::{resolve_output, stage_unsigned};
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
    let mut profiler = PatchProfiler::new(opening);
    let result = execute(request, &trust, opened, bundles, &mut emit, &mut profiler);
    release_process_memory();
    let (results, output) = result?;
    Ok(PatchOutcome {
        output,
        results,
        metrics: profiler.finish(),
    })
}

fn execute(
    request: &PatchRequest,
    trust: &TrustStore,
    mut opened: OpenedApk,
    bundles: Vec<OpenedBundle>,
    emit: &mut impl FnMut(RunEvent),
    profiler: &mut PatchProfiler,
) -> Result<(Vec<PatchResult>, PatchArtifact)> {
    emit(info(format!("Opening APK {}", request.apk_path)));
    report_container(&opened, emit);
    let output = resolve_output(&request.output, opened.apk.components().len())?;

    emit(info("Loading bundles".to_string()));
    let archives = trusted_archives(bundles, trust)?;
    let package = opened.apk.package_name().map(Cow::into_owned);
    let version_name = opened.apk.version_name().map(Cow::into_owned);
    let version = ApkVersion {
        name: version_name.as_deref(),
        code: opened.apk.version_code(),
    };
    let specs: Vec<&PatchSpec> = archives.iter().flat_map(TrustedArchive::catalog).collect();
    let plan = ResolvedPlan::resolve(
        &specs,
        &request.selection,
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

    let bundles = load_executed(archives, &plan, profiler)?;
    let mut applied = apply(&mut opened, &bundles, plan, emit, profiler)?;
    if !request.leave_out_failed {
        ensure_none_failed(applied.results())?;
    }

    emit(info(format!(
        "Writing signed output to {}",
        output.path().display()
    )));
    release_for_writing(&mut opened.apk);
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

fn delivery(method: InstallMethod) -> Delivery {
    match method {
        InstallMethod::Install => Delivery::Install,
        InstallMethod::Mount => Delivery::Mount,
    }
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
