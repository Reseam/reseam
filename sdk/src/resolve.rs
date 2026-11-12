// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::analysis::{PatchCheck, ResolveReport, ResolveRequest, TargetResolution};
use reseam_model::{PatchPreset, PatchSelection, PatchSpec, PatchStatus};
use reseam_patcher::engine::{Delivery, PlanOptions, ResolvedPlan, Trace};
use reseam_patcher::inspect::TargetCheck;

use crate::apply::{apply, load_executed};
use crate::bundles::{TrustedArchive, trusted_archives};
use crate::error::Result;
use crate::inputs::Inputs;
use crate::progress::RunProgress;
use crate::trust::TrustStore;

/// Runs every listed patch of the bundles that declares the APK's package or
/// suits any app, with default options and version checks off, and reports
/// how each target the patches used resolved. A patch whose required option
/// has no default is left out, as it cannot run without one. The patched APK
/// is never written.
pub fn resolve(request: &ResolveRequest) -> Result<ResolveReport> {
    let trust = TrustStore::new(&request.trust)?;
    let Inputs {
        apk,
        opening,
        bundles,
    } = Inputs::open(
        Some(&request.apk_path),
        &request.split_paths,
        &[],
        &request.bundle_paths,
    )?;
    let mut opened = apk.expect("resolve inputs are opened with the request's APK");
    let archives = trusted_archives(bundles, &trust)?;
    let identity = opened.identity();
    let package = identity.package_name.as_deref();
    let specs: Vec<&PatchSpec> = archives.iter().flat_map(TrustedArchive::catalog).collect();
    let checked: Vec<PatchSpec> = specs
        .iter()
        .filter(|spec| spec.listed_for(package))
        .map(|spec| (*spec).clone())
        .collect();
    let selection = PatchSelection {
        preset: PatchPreset::None,
        enable: checked
            .iter()
            .filter(|spec| !needs_an_option(spec))
            .map(|spec| spec.reference().to_string())
            .collect(),
        disable: specs
            .iter()
            .filter(|spec| needs_an_option(spec))
            .map(|spec| spec.reference().to_string())
            .collect(),
        ignore_versions: true,
        ..PatchSelection::default()
    };
    let plan = ResolvedPlan::resolve(
        &specs,
        &selection,
        Delivery::Install,
        package,
        identity.version(),
    )?;
    let options = PlanOptions::resolve(&specs, &selection, &plan, package)?;
    let mut ignore = |_| {};
    let mut progress = RunProgress::new(opening, &mut ignore);
    let check = TargetCheck::start()?;
    let bundles = load_executed(archives, &plan, &mut progress)?;
    let applied = apply(
        &mut opened,
        &bundles,
        plan,
        &options,
        Trace::Off,
        &mut progress,
    )?;
    let found = check.finish()?;
    let results = applied.into_results();
    let patches = checked
        .into_iter()
        .map(|spec| {
            let reference = spec.reference();
            let status = if needs_an_option(&spec) {
                skipped_for_option(&spec)
            } else {
                results
                    .iter()
                    .find(|result| result.patch == reference)
                    .expect("every enabled patch is a root, so the run reports it")
                    .status
                    .clone()
            };
            PatchCheck {
                status,
                resolutions: resolutions_of(&found.resolutions, &reference),
                patch: reference,
                name: spec.name.clone(),
                compatibility: spec.compatibility.clone(),
            }
        })
        .collect();
    Ok(ResolveReport {
        patches,
        unreached: found.unreached,
    })
}

/// Whether the patch declares a required option with no default.
fn needs_an_option(spec: &PatchSpec) -> bool {
    spec.options
        .iter()
        .any(|option| option.required && option.default_value.is_none())
}

fn skipped_for_option(spec: &PatchSpec) -> PatchStatus {
    let keys: Vec<&str> = spec
        .options
        .iter()
        .filter(|option| option.required && option.default_value.is_none())
        .map(|option| option.key.as_str())
        .collect();
    PatchStatus::Skipped {
        reason: format!("needs a value for {}", keys.join(", ")),
    }
}

fn resolutions_of(
    resolutions: &[TargetResolution],
    patch: &reseam_model::PatchRef,
) -> Vec<TargetResolution> {
    resolutions
        .iter()
        .filter(|resolution| resolution.patch.as_ref() == Some(patch))
        .cloned()
        .collect()
}
