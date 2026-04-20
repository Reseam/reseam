// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use reseam_apk::{ApkFile, ApkWriteOptions};
use reseam_patcher::bundle::{BundleKeepAlive, PatchBundle};
use reseam_patcher::context::PatchContext;
use reseam_patcher::engine::{self, ExecutionPlan, PatchStatus, ProgressEvent};
use reseam_patcher::options::{OptionDeclaration, OptionValue, PatchOptions};
use reseam_sign::{GeneratedKey, SigningKey};
use tracing::info;

use crate::inspect::{load_bundle_with_trust, open_apk};
use crate::types::{
    ArtifactKind, InputOptionValue, PatchArtifact, PatchOutcome, PatchOutput, PatchRequest,
    PatchRunStatus, PatchSelection, RunEvent,
};

pub fn parse_cli_option(option: &str) -> Result<(String, String, String)> {
    let (lhs, value) = option
        .split_once('=')
        .with_context(|| format!("invalid option '{option}': expected PATCH.KEY=VALUE"))?;
    let (patch_name, option_key) = lhs
        .split_once('.')
        .with_context(|| format!("invalid option '{option}': expected PATCH.KEY=VALUE"))?;
    if patch_name.is_empty() || option_key.is_empty() {
        bail!("invalid option '{option}': patch and key must be non-empty");
    }

    Ok((patch_name.to_string(), option_key.to_string(), value.to_string()))
}

pub fn selection_from_cli(
    enable: &[String],
    disable: &[String],
    option_args: &[String],
    bundle: &PatchBundle,
) -> Result<PatchSelection> {
    let mut selection = PatchSelection {
        enable: enable.to_vec(),
        disable: disable.to_vec(),
        options: Default::default(),
    };

    for raw in option_args {
        let (patch_name, option_key, value) = parse_cli_option(raw)?;
        let declaration = find_option_declaration(&bundle.patches, &patch_name, &option_key)?;
        let parsed = declaration
            .parse_value(&value)
            .with_context(|| format!("failed to parse --option {raw}"))?;
        selection
            .options
            .entry(patch_name)
            .or_default()
            .insert(option_key, InputOptionValue::from(&parsed));
    }

    Ok(selection)
}

pub fn build_execution_plan(
    patches: &[Box<dyn reseam_patcher::patch::Patch>],
    selection: &PatchSelection,
) -> Result<ExecutionPlan> {
    let mut plan = ExecutionPlan::new();

    for patch in &selection.enable {
        plan.select_patch(patch.clone());
    }
    for patch in &selection.disable {
        plan.disable_patch(patch.clone());
    }

    for (patch_name, options) in &selection.options {
        let mut patch_options = PatchOptions::new();
        for (option_key, value) in options {
            let declaration = find_option_declaration(patches, patch_name, option_key)?;
            patch_options.set(
                option_key.clone(),
                input_to_option_value(value, declaration)
                    .with_context(|| format!("invalid option {patch_name}.{option_key}"))?,
            );
        }
        plan.set_patch_options(patch_name.clone(), patch_options);
    }

    Ok(plan)
}

pub fn patch<F>(request: &PatchRequest, mut emit: F) -> Result<PatchOutcome>
where
    F: FnMut(RunEvent),
{
    emit(RunEvent::Info {
        message: format!("Opening APK {}", request.apk_path.display()),
    });
    let mut apk = open_apk(&request.apk_path, &request.split_paths)?;

    if request.bundle_paths.is_empty() {
        bail!("at least one bundle is required");
    }

    let mut loaded_bundles = Vec::with_capacity(request.bundle_paths.len());
    for bundle_path in &request.bundle_paths {
        emit(RunEvent::Info {
            message: format!("Loading bundle {}", bundle_path.display()),
        });
        loaded_bundles.push(load_bundle_with_trust(bundle_path, &request.trust_store)?);
    }

    let aggregate_bundle = aggregate_bundles(loaded_bundles);
    let plan = build_execution_plan(&aggregate_bundle.patches, &request.selection)?;

    if request.dry_run {
        let results = engine::validate_patches_with_plan(
            &aggregate_bundle.patches,
            &plan,
            apk.package_name(),
            apk.version_name(),
        )
        .context("patch validation failed")?;
        for result in &results {
            emit(status_event(result));
        }
        return Ok(PatchOutcome {
            results,
            artifact: None,
        });
    }

    let mut ctx = PatchContext::new(&mut apk);
    let results = engine::apply_patches_with_plan_and_observer(
        &mut ctx,
        &aggregate_bundle.patches,
        &plan,
        |event| match event {
            ProgressEvent::PatchStarted { patch } => emit(RunEvent::PatchStarted { patch }),
            ProgressEvent::PatchLog(log) => emit(RunEvent::PatchLog {
                patch: log.patch,
                level: log.level.to_string(),
                message: log.message,
            }),
            ProgressEvent::PatchFinished { patch, status } => emit(RunEvent::PatchFinished {
                patch,
                reason: patch_status_reason(&status),
                status: patch_run_status(&status),
            }),
        },
    )
    .context("patch application failed")?;
    drop(ctx);

    let failed_count = results
        .iter()
        .filter(|result| matches!(result.status, PatchStatus::Failed { .. }))
        .count();
    if failed_count > 0 {
        bail!("{failed_count} patch(es) failed");
    }

    let artifact = match &request.output {
        PatchOutput::SingleFile(output_path) => {
            emit(RunEvent::Info {
                message: format!("Writing signed APK to {}", output_path.display()),
            });
            write_signed_single_apk(
                &mut apk,
                output_path,
                request.key_path.as_deref(),
                request.cert_path.as_deref(),
            )?;
            PatchArtifact {
                kind: ArtifactKind::Apk,
                path: output_path.clone(),
            }
        }
        PatchOutput::SplitDir(output_dir) => {
            emit(RunEvent::Info {
                message: format!("Writing signed split APK set to {}", output_dir.display()),
            });
            write_signed_split_apks(
                &mut apk,
                output_dir,
                request.key_path.as_deref(),
                request.cert_path.as_deref(),
            )?;
            PatchArtifact {
                kind: ArtifactKind::SplitDirectory,
                path: output_dir.clone(),
            }
        }
    };

    Ok(PatchOutcome {
        results,
        artifact: Some(artifact),
    })
}

struct AggregateBundle {
    patches: Vec<Box<dyn reseam_patcher::patch::Patch>>,
    _keepers: Vec<BundleKeepAlive>,
}

fn aggregate_bundles(bundles: Vec<PatchBundle>) -> AggregateBundle {
    let mut patches = Vec::new();
    let mut keepers = Vec::with_capacity(bundles.len());

    for bundle in bundles {
        let (mut bundle_patches, keeper) = bundle.into_patches_and_keepalive();
        patches.append(&mut bundle_patches);
        keepers.push(keeper);
    }

    AggregateBundle {
        patches,
        _keepers: keepers,
    }
}

fn input_to_option_value(value: &InputOptionValue, declaration: &OptionDeclaration) -> Result<OptionValue> {
    let value = match value {
        InputOptionValue::String(value) => OptionValue::String(value.clone()),
        InputOptionValue::Bool(value) => OptionValue::Bool(*value),
        InputOptionValue::Int(value) => OptionValue::Int(*value),
        InputOptionValue::Float(value) => OptionValue::Float(*value),
        InputOptionValue::StringList(value) => OptionValue::StringList(value.clone()),
        InputOptionValue::Path(value) => OptionValue::Path(PathBuf::from(value)),
    };
    declaration.validate_value(&value)?;
    Ok(value)
}

fn find_option_declaration<'a>(
    patches: &'a [Box<dyn reseam_patcher::patch::Patch>],
    patch_name: &str,
    option_key: &str,
) -> Result<&'a OptionDeclaration> {
    let patch = patches
        .iter()
        .find(|patch| patch.name() == patch_name)
        .with_context(|| format!("unknown patch '{patch_name}'"))?;
    patch
        .options()
        .iter()
        .find(|declaration| declaration.key == option_key)
        .with_context(|| format!("unknown option '{option_key}' for patch '{patch_name}'"))
}

fn status_event(result: &engine::PatchResult) -> RunEvent {
    RunEvent::PatchFinished {
        patch: result.name.clone(),
        status: patch_run_status(&result.status),
        reason: patch_status_reason(&result.status),
    }
}

fn patch_run_status(status: &PatchStatus) -> PatchRunStatus {
    match status {
        PatchStatus::Applied => PatchRunStatus::Applied,
        PatchStatus::Skipped { .. } => PatchRunStatus::Skipped,
        PatchStatus::Failed { .. } => PatchRunStatus::Failed,
    }
}

fn patch_status_reason(status: &PatchStatus) -> Option<String> {
    match status {
        PatchStatus::Applied => None,
        PatchStatus::Skipped { reason } | PatchStatus::Failed { reason } => Some(reason.clone()),
    }
}

fn write_signed_single_apk(
    apk: &mut ApkFile,
    output_path: &Path,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
) -> Result<()> {
    if let Some(parent) = output_path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }

    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    apk.write_to_with_options(
        tmp_dir.path(),
        ApkWriteOptions {
            strip_signatures: true,
        },
    )
    .context("failed to write patched APK")?;

    let tmp_apk_path = find_output_apks(tmp_dir.path())?
        .into_iter()
        .next()
        .context("no APK file found in output directory")?;
    let signing_key = load_or_generate_key(
        output_path.with_extension("pk8"),
        output_path.with_extension("der"),
        key_path,
        cert_path,
    )?;
    sign_apk_to_path(&tmp_apk_path, output_path, &signing_key)
}

fn write_signed_split_apks(
    apk: &mut ApkFile,
    output_dir: &Path,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;

    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    apk.write_to_with_options(
        tmp_dir.path(),
        ApkWriteOptions {
            strip_signatures: true,
        },
    )
    .context("failed to write patched split APK set")?;

    let signing_key = load_or_generate_key(
        output_dir.join("reseam.pk8"),
        output_dir.join("reseam.der"),
        key_path,
        cert_path,
    )?;

    for unsigned_apk in find_output_apks(tmp_dir.path())? {
        let file_name = unsigned_apk
            .file_name()
            .context("temporary APK output is missing a filename")?;
        let output_path = output_dir.join(file_name);
        sign_apk_to_path(&unsigned_apk, &output_path, &signing_key)?;
    }

    Ok(())
}

fn find_output_apks(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut apks = Vec::new();
    for entry in std::fs::read_dir(dir).context("failed to read temp directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "apk") {
            apks.push(path);
        }
    }
    apks.sort();
    Ok(apks)
}

fn sign_apk_to_path(unsigned_path: &Path, output_path: &Path, signing_key: &SigningKey) -> Result<()> {
    let unsigned_bytes = std::fs::read(unsigned_path)
        .with_context(|| format!("failed to read {}", unsigned_path.display()))?;
    let signed_bytes =
        reseam_sign::v2::sign(&unsigned_bytes, signing_key).context("v2 signing failed")?;
    std::fs::write(output_path, &signed_bytes)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    info!(output_path = %output_path.display(), "patched APK written");
    Ok(())
}

fn load_or_generate_key(
    default_key_path: PathBuf,
    default_cert_path: PathBuf,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
) -> Result<SigningKey> {
    let key_path = key_path.map(Path::to_path_buf);
    let cert_path = cert_path.map(Path::to_path_buf);
    let (key_path, cert_path) = match (key_path, cert_path) {
        (Some(key), Some(cert)) => (key, cert),
        (None, None) => (default_key_path, default_cert_path),
        _ => bail!("key and cert must both be provided"),
    };

    if !(key_path.exists() && cert_path.exists()) {
        let generated = GeneratedKey::generate().context("failed to generate signing key")?;
        generated
            .save(&key_path, &cert_path)
            .context("failed to save signing key")?;
    }

    let key_bytes = std::fs::read(&key_path)
        .with_context(|| format!("failed to read key {}", key_path.display()))?;
    let cert_bytes = std::fs::read(&cert_path)
        .with_context(|| format!("failed to read cert {}", cert_path.display()))?;
    SigningKey::from_pkcs8(&key_bytes, cert_bytes).context("failed to load signing key")
}
