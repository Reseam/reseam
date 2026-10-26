// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use anyhow::{Context, Result, anyhow, ensure};
use reseam_model::{
    InspectRequest, InstallMethod, LogLevel, OptionDeclaration, OptionType, OptionValue,
    PatchMetadata, PatchOutcome, PatchOutput, PatchRequest, PatchResult, PatchSelection,
    PatchStatus, RunEvent, SigningKeyFiles,
};
use reseam_sdk::{HostError, PreparedInspection};
use tracing::{debug, error, info, warn};

use crate::app::{PatchCommand, PatchRequestArgs};

pub fn run_patch(command: &PatchCommand) -> Result<()> {
    let apk = &command.request.apk;
    let output = if let Some(path) = &command.output {
        PatchOutput::SingleFile {
            path: path.display().to_string(),
        }
    } else if let Some(path) = &command.output_dir {
        PatchOutput::SplitDir {
            path: path.display().to_string(),
        }
    } else {
        let stem = apk
            .file_stem()
            .context("invalid APK path")?
            .to_string_lossy();
        PatchOutput::Auto {
            path: apk
                .with_file_name(format!("{stem}-patched"))
                .display()
                .to_string(),
        }
    };

    let dry_run = command.request.dry_run;
    let outcome = run(
        request(&command.request, output),
        &command.request.option,
        log_event,
    )
    .map_err(|error| {
        if error.downcast_ref::<HostError>().is_some_and(|error| {
            matches!(
                error,
                HostError::Problem(reseam_model::Problem::SingleFileComponents { .. })
            )
        }) {
            error.context("use --output-dir for split input")
        } else {
            error
        }
    })?;
    let count =
        |wanted: fn(&PatchResult) -> bool| outcome.results.iter().filter(|r| wanted(r)).count();
    info!(
        applied = count(|result| applied(result) && result.chosen()),
        dependencies = count(|result| applied(result) && !result.chosen()),
        skipped = count(|result| matches!(result.status, PatchStatus::Skipped { .. })),
        unmountable = count(|result| matches!(result.status, PatchStatus::Unmountable { .. })),
        failed = count(|result| matches!(result.status, PatchStatus::Failed { .. })),
        "patch run finished"
    );
    if dry_run {
        info!("dry run: validation completed without applying patches");
    } else {
        info!(path = %outcome.output.path().display(), "patched output ready");
    }
    Ok(())
}

fn applied(result: &PatchResult) -> bool {
    matches!(result.status, PatchStatus::Applied)
}

/// Internal patches only show their problems unless debug logging is on.
fn log_event(event: RunEvent, hidden: &HashSet<String>) {
    match event {
        RunEvent::Info { message } => info!(message),
        RunEvent::PatchStarted { patch } if hidden.contains(&patch) => {
            debug!(patch, "patch started");
        }
        RunEvent::PatchStarted { patch } => info!(patch, "patch started"),
        RunEvent::PatchFinished { patch, status } => match status {
            PatchStatus::Applied if hidden.contains(&patch) => debug!(patch, "patch applied"),
            PatchStatus::Applied => info!(patch, "patch applied"),
            PatchStatus::Skipped { reason } => warn!(patch, reason, "patch skipped"),
            PatchStatus::Unmountable { reason } => info!(patch, reason, "patch left out of mount"),
            PatchStatus::Failed { reason } => error!(patch, reason, "patch failed"),
        },
        RunEvent::PatchesReverted { patches } => {
            info!(?patches, "patches undone to run again");
        }
        RunEvent::PatchLog(entry) => match entry.level {
            LogLevel::Debug => debug!(patch = entry.patch, "{}", entry.message),
            LogLevel::Info => info!(patch = entry.patch, "{}", entry.message),
            LogLevel::Warn => warn!(patch = entry.patch, "{}", entry.message),
        },
    }
}

pub(crate) fn request(args: &PatchRequestArgs, output: PatchOutput) -> PatchRequest {
    let selection = PatchSelection {
        preset: args.preset,
        enable: args.enable.clone(),
        disable: args.disable.clone(),
        ignore_versions: args.ignore_versions,
        ..Default::default()
    };
    PatchRequest {
        apk_path: args.apk.display().to_string(),
        split_paths: args
            .split
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        bundle_paths: args
            .bundle
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        trust: args.trust.trust(),
        selection,
        output,
        signing: args
            .key
            .as_ref()
            .zip(args.cert.as_ref())
            .map(|(key, cert)| SigningKeyFiles {
                key: key.display().to_string(),
                cert: cert.display().to_string(),
            }),
        dry_run: args.dry_run,
        install_method: if args.mount {
            InstallMethod::Mount
        } else {
            InstallMethod::Install
        },
        leave_out_failed: args.leave_out_failed,
    }
}

/// Inspects the request's inputs, sets `options` (`PATCH.KEY=VALUE`) on the
/// patches they name, and runs it. `emit` also gets the references of the
/// run's hidden patches.
pub(crate) fn run(
    request: PatchRequest,
    options: &[String],
    mut emit: impl FnMut(RunEvent, &HashSet<String>),
) -> Result<PatchOutcome> {
    let prepared = PreparedInspection::open(&InspectRequest {
        apk_path: Some(request.apk_path.clone()),
        split_paths: request.split_paths.clone(),
        bundle_paths: request.bundle_paths.clone(),
        trust: request.trust.clone(),
    })?;
    let hidden: HashSet<String> = prepared
        .metadata()
        .patches
        .iter()
        .filter(|patch| patch.spec.hidden)
        .map(|patch| patch.spec.reference())
        .collect();
    let request = PatchRequest {
        selection: with_options(request.selection, options, &prepared)?,
        ..request
    };
    Ok(prepared.patch(&request, |event| emit(event, &hidden))?)
}

fn with_options(
    mut selection: PatchSelection,
    options: &[String],
    prepared: &PreparedInspection,
) -> Result<PatchSelection> {
    for raw in options {
        let invalid = || anyhow!("invalid option '{raw}': expected PATCH.KEY=VALUE");
        let (lhs, value) = raw.split_once('=').ok_or_else(invalid)?;
        let (patch, key) = option_target(prepared, lhs)?;
        let reference = patch.spec.reference();
        let declaration = patch
            .spec
            .options
            .iter()
            .find(|declaration| declaration.key == key)
            .with_context(|| format!("unknown option '{key}' for patch '{reference}'"))?;
        let value =
            parse_option(declaration, value).with_context(|| format!("invalid --option {raw}"))?;
        selection
            .options
            .entry(reference)
            .or_default()
            .insert(key.to_owned(), value);
    }
    Ok(selection)
}

/// Splits `PATCH.KEY` at the last dot that leaves a known patch, since patch
/// IDs and names contain dots themselves.
fn option_target<'p, 'a>(
    prepared: &'p PreparedInspection,
    lhs: &'a str,
) -> Result<(&'p PatchMetadata, &'a str)> {
    for (separator, _) in lhs.rmatch_indices('.') {
        let (selector, key) = (&lhs[..separator], &lhs[separator + 1..]);
        ensure!(
            !selector.is_empty() && !key.is_empty(),
            "invalid option '{lhs}': expected PATCH.KEY"
        );
        if let Some(patch) = prepared.resolve_patch(selector)? {
            return Ok((patch, key));
        }
    }
    Err(anyhow!(
        "unknown patch in option '{lhs}'; expected PATCH.KEY (use <bundle>/<id>, an ID, or an unambiguous name)"
    ))
}

fn parse_option(declaration: &OptionDeclaration, raw: &str) -> Result<OptionValue> {
    Ok(match declaration.option_type {
        OptionType::String => OptionValue::Text(raw.to_owned()),
        OptionType::Bool => OptionValue::Bool(raw.parse().context("expected bool")?),
        OptionType::Int => OptionValue::Int(raw.parse().context("expected int")?),
        OptionType::Float => OptionValue::Float(raw.parse().context("expected float")?),
        OptionType::StringList => OptionValue::TextList(
            raw.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect(),
        ),
        OptionType::Path => OptionValue::Path(raw.to_owned()),
    })
}
