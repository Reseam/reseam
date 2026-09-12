// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{anyhow, ensure, Context, Result};
use reseam_patcher::engine::{PatchIndex, PatchResult, PatchSelection, PatchStatus};
use reseam_patcher::error::PatcherError;
use reseam_sdk::{
    inspect_apk, load_bundles, patch, PatchOutput, PatchRequest, RunEvent, SigningKeyFiles,
    TrustStore,
};
use tracing::{error, info, warn};

use crate::app::{PatchCommand, PatchRequestArgs};

pub fn run_patch(command: &PatchCommand) -> Result<()> {
    let apk = &command.request.apk;
    let output = if let Some(path) = &command.output {
        PatchOutput::SingleFile { path: path.clone() }
    } else if let Some(path) = &command.output_dir {
        PatchOutput::SplitDir { path: path.clone() }
    } else {
        let stem = apk
            .file_stem()
            .context("invalid APK path")?
            .to_string_lossy();
        PatchOutput::Auto {
            path: apk.with_file_name(format!("{stem}-patched")),
        }
    };

    let request = request(&command.request, output)?;
    let outcome = patch(&request, log_event)?;
    let count =
        |wanted: fn(&PatchResult) -> bool| outcome.results.iter().filter(|r| wanted(r)).count();
    info!(
        applied = count(|result| applied(result) && result.chosen()),
        dependencies = count(|result| applied(result) && !result.chosen()),
        skipped = count(|result| matches!(result.status, PatchStatus::Skipped { .. })),
        failed = count(|result| matches!(result.status, PatchStatus::Failed { .. })),
        "patch run finished"
    );
    if request.dry_run {
        info!("dry run: validation completed without applying patches");
    } else {
        info!(path = %outcome.output.path().display(), "patched output ready");
    }
    Ok(())
}

fn applied(result: &PatchResult) -> bool {
    matches!(result.status, PatchStatus::Applied)
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
        ignore_versions: args.ignore_versions,
        ..Default::default()
    };
    if args.option.is_empty() {
        return Ok(selection);
    }
    let bundles = load_bundles(std::slice::from_ref(&args.bundle), trust)?;
    let patches: Vec<_> = bundles
        .iter()
        .flat_map(|bundle| bundle.patches.iter().map(Box::as_ref))
        .collect();
    let index = PatchIndex::new(&patches)?;
    let apk = inspect_apk(&args.apk, &args.split)?;
    for raw in &args.option {
        let invalid = || anyhow!("invalid option '{raw}': expected PATCH.KEY=VALUE");
        let (lhs, value) = raw.split_once('=').ok_or_else(invalid)?;
        let (patch_index, key) = option_target(&index, lhs, apk.package_name.as_deref())?;
        let patch = patches[patch_index].id();
        let declaration = patches[patch_index]
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

/// IDs and option keys may contain dots. Resolve the longest recognized patch
/// prefix instead of assuming the first dot separates the patch from its option.
fn option_target<'a>(
    index: &PatchIndex<'_>,
    lhs: &'a str,
    package: Option<&str>,
) -> Result<(usize, &'a str)> {
    for (separator, _) in lhs.rmatch_indices('.') {
        let (selector, key) = (&lhs[..separator], &lhs[separator + 1..]);
        ensure!(
            !selector.is_empty() && !key.is_empty(),
            "invalid option '{lhs}': expected PATCH.KEY"
        );
        match index.resolve(selector, package) {
            Ok(patch) => return Ok((patch, key)),
            Err(PatcherError::UnknownPatch(_)) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(anyhow!(
        "unknown patch in option '{lhs}'; expected PATCH.KEY (use a patch ID or unambiguous name)"
    ))
}
