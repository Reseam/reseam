// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use reseam_library::{
    built_in_trust_store, load_bundle_with_trust, patch as patch_with_library, selection_from_cli,
    ArtifactKind, PatchOutput as LibraryPatchOutput, PatchRequest, PatchRunStatus, RunEvent,
};
use reseam_patcher::engine::PatchStatus;
use tracing::{error, info, warn};

use crate::app::{PatchCommand, PatchRequestArgs};

pub(crate) enum OutputTarget {
    SingleFile(PathBuf),
    SplitDir(PathBuf),
}

pub fn run_patch(command: &PatchCommand) -> Result<()> {
    let split_mode = !command.request.split.is_empty();
    if split_mode && command.output.is_some() {
        bail!("--output cannot be used with --split; use --output-dir instead");
    }
    if !split_mode && command.output_dir.is_some() {
        bail!("--output-dir can only be used with --split");
    }

    let output_target = if split_mode {
        let dir = match &command.output_dir {
            Some(dir) => dir.clone(),
            None => {
                let stem = command
                    .request
                    .apk
                    .file_stem()
                    .context("invalid APK path")?
                    .to_string_lossy();
                command
                    .request
                    .apk
                    .with_file_name(format!("{stem}-patched"))
            }
        };
        OutputTarget::SplitDir(dir)
    } else {
        let path = match &command.output {
            Some(path) => path.clone(),
            None => {
                let stem = command
                    .request
                    .apk
                    .file_stem()
                    .context("invalid APK path")?
                    .to_string_lossy();
                command
                    .request
                    .apk
                    .with_file_name(format!("{stem}-patched.apk"))
            }
        };
        OutputTarget::SingleFile(path)
    };

    let request = build_patch_request(&command.request, patch_output(output_target))?;

    let outcome = patch_with_library(&request, |event| match event {
        RunEvent::Info { message } => info!(message),
        RunEvent::PatchStarted { patch } => info!(patch, "patch started"),
        RunEvent::PatchFinished {
            patch,
            status,
            reason,
        } => match status {
            PatchRunStatus::Applied => info!(patch, "patch completed"),
            PatchRunStatus::Skipped => {
                warn!(patch, reason = reason.unwrap_or_default(), "patch skipped")
            }
            PatchRunStatus::Failed => {
                error!(patch, reason = reason.unwrap_or_default(), "patch failed")
            }
        },
        RunEvent::PatchLog {
            patch,
            level,
            message,
        } => info!(patch, level, message, "patch log"),
    })?;

    let failed_count = outcome
        .results
        .iter()
        .filter(|result| matches!(result.status, PatchStatus::Failed { .. }))
        .count();

    if command.request.dry_run {
        if failed_count > 0 {
            bail!("{failed_count} patch(es) failed validation");
        }
        info!("dry run enabled; validation completed without applying patches");
        return Ok(());
    }

    if let Some(artifact) = outcome.artifact {
        match artifact.kind {
            ArtifactKind::Apk => info!(path = %artifact.path.display(), "patched APK ready"),
            ArtifactKind::SplitDirectory => {
                info!(path = %artifact.path.display(), "patched split APK set ready")
            }
        }
    }

    Ok(())
}

pub(crate) fn build_patch_request(
    args: &PatchRequestArgs,
    output: LibraryPatchOutput,
) -> Result<PatchRequest> {
    let trust_store = built_in_trust_store();
    let patch_bundle = load_bundle_with_trust(&args.bundle, &trust_store)?;
    let selection = selection_from_cli(&args.enable, &args.disable, &args.option, &patch_bundle)?;

    Ok(PatchRequest {
        apk_path: args.apk.clone(),
        split_paths: args.split.clone(),
        bundle_paths: vec![args.bundle.clone()],
        trust_store,
        selection,
        output,
        key_path: args.key.clone(),
        cert_path: args.cert.clone(),
        dry_run: args.dry_run,
    })
}

pub(crate) fn patch_output(target: OutputTarget) -> LibraryPatchOutput {
    match target {
        OutputTarget::SingleFile(path) => LibraryPatchOutput::SingleFile(path),
        OutputTarget::SplitDir(path) => LibraryPatchOutput::SplitDir(path),
    }
}
