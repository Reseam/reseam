// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{anyhow, ensure, Context, Result};
use reseam_patcher::engine::{PatchSelection, PatchStatus};
use reseam_sdk::{
    load_bundles, patch, PatchOutput, PatchRequest, RunEvent, SigningKeyFiles, TrustStore,
};
use tracing::{error, info, warn};

use crate::app::{PatchCommand, PatchRequestArgs};

pub fn run_patch(command: &PatchCommand) -> Result<()> {
    let apk = &command.request.apk;
    let stem = apk
        .file_stem()
        .context("invalid APK path")?
        .to_string_lossy();
    let output = if command.request.split.is_empty() {
        PatchOutput::SingleFile {
            path: command
                .output
                .clone()
                .unwrap_or_else(|| apk.with_file_name(format!("{stem}-patched.apk"))),
        }
    } else {
        PatchOutput::SplitDir {
            path: command
                .output_dir
                .clone()
                .unwrap_or_else(|| apk.with_file_name(format!("{stem}-patched"))),
        }
    };

    let request = request(&command.request, output)?;
    let outcome = patch(&request, log_event)?;
    let count = |wanted: fn(&PatchStatus) -> bool| {
        outcome
            .results
            .iter()
            .filter(|result| wanted(&result.status))
            .count()
    };
    info!(
        applied = count(|status| matches!(status, PatchStatus::Applied)),
        skipped = count(|status| matches!(status, PatchStatus::Skipped { .. })),
        failed = count(|status| matches!(status, PatchStatus::Failed { .. })),
        "patch run finished"
    );
    if request.dry_run {
        info!("dry run: validation completed without applying patches");
    } else {
        info!(path = %request.output.path().display(), "patched output ready");
    }
    Ok(())
}

fn log_event(event: RunEvent) {
    match event {
        RunEvent::Info { message } => info!(message),
        RunEvent::PatchStarted { patch } => info!(patch, "patch started"),
        RunEvent::PatchFinished { patch, status } => match status {
            PatchStatus::Applied => info!(patch, "patch applied"),
            PatchStatus::Skipped { reason } => warn!(patch, reason, "patch skipped"),
            PatchStatus::Failed { reason } => error!(patch, reason, "patch failed"),
        },
        RunEvent::PatchLog(entry) => info!(
            patch = entry.patch,
            level = %entry.level,
            entry.message,
            "patch log"
        ),
    }
}

pub(crate) fn request(args: &PatchRequestArgs, output: PatchOutput) -> Result<PatchRequest> {
    let trust = args.trust.store()?;
    let selection = selection(args, &trust)?;
    Ok(PatchRequest {
        apk_path: args.apk.clone(),
        split_paths: args.split.clone(),
        bundle_paths: vec![args.bundle.clone()],
        trust,
        selection,
        output,
        signing: args
            .key
            .clone()
            .zip(args.cert.clone())
            .map(|(key, cert)| SigningKeyFiles { key, cert }),
        dry_run: args.dry_run,
    })
}

/// `--option PATCH.KEY=VALUE` values are typed by the patch's declaration,
/// which means loading the bundle once up front.
fn selection(args: &PatchRequestArgs, trust: &TrustStore) -> Result<PatchSelection> {
    let mut selection = PatchSelection {
        enable: args.enable.iter().cloned().collect(),
        disable: args.disable.iter().cloned().collect(),
        ..Default::default()
    };
    if args.option.is_empty() {
        return Ok(selection);
    }
    let bundles = load_bundles(std::slice::from_ref(&args.bundle), trust)?;
    for raw in &args.option {
        let invalid = || anyhow!("invalid option '{raw}': expected PATCH.KEY=VALUE");
        let (lhs, value) = raw.split_once('=').ok_or_else(invalid)?;
        let (patch, key) = lhs.split_once('.').ok_or_else(invalid)?;
        ensure!(!patch.is_empty() && !key.is_empty(), invalid());
        let declaration = bundles
            .iter()
            .flat_map(|bundle| &bundle.patches)
            .find(|candidate| candidate.name() == patch)
            .with_context(|| format!("unknown patch '{patch}'"))?
            .spec()
            .options
            .iter()
            .find(|declaration| declaration.key == key)
            .with_context(|| format!("unknown option '{key}' for patch '{patch}'"))?;
        let value = declaration
            .parse(value)
            .map_err(|reason| anyhow!("invalid --option {raw}: {reason}"))?;
        selection
            .options
            .entry(patch.to_string())
            .or_default()
            .set(key, value);
    }
    Ok(selection)
}
