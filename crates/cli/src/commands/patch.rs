// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::RefCell;
use std::collections::HashSet;

use anyhow::{Context, Result, anyhow, ensure};
use reseam_patcher::engine::{PatchIndex, PatchResult, PatchSelection, PatchStatus};
use reseam_patcher::error::PatcherError;
use reseam_patcher::log::LogLevel;
use reseam_patcher::options::{OptionDeclaration, OptionType, OptionValue};
use reseam_sdk::{
    HostError, PatchOutput, PatchRequest, RunEvent, SigningKeyFiles, patch_with_selection,
};
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

    let request = request(&command.request, output)?;
    let outcome = run(&request, &command.request.option, log_event).map_err(|error| {
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
            PatchStatus::Failed { reason } => error!(patch, reason, "patch failed"),
        },
        RunEvent::PatchLog(entry) => match entry.level {
            LogLevel::Debug => debug!(patch = entry.patch, "{}", entry.message),
            LogLevel::Info => info!(patch = entry.patch, "{}", entry.message),
            LogLevel::Warn => warn!(patch = entry.patch, "{}", entry.message),
        },
    }
}

pub(crate) fn request(args: &PatchRequestArgs, output: PatchOutput) -> Result<PatchRequest> {
    let trust = args.trust.store()?;
    let selection = PatchSelection {
        preset: args.preset,
        enable: args.enable.clone(),
        disable: args.disable.clone(),
        ignore_versions: args.ignore_versions,
        ..Default::default()
    };
    Ok(PatchRequest {
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
        trust: (&trust).into(),
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
    })
}

/// `emit` also gets the references of the run's hidden patches.
pub(crate) fn run(
    request: &PatchRequest,
    options: &[String],
    mut emit: impl FnMut(RunEvent, &HashSet<String>),
) -> Result<reseam_sdk::PatchOutcome> {
    let hidden = RefCell::new(HashSet::new());
    Ok(patch_with_selection(
        request,
        |specs, package| {
            hidden.replace(
                specs
                    .iter()
                    .filter(|spec| spec.hidden)
                    .map(|spec| spec.reference())
                    .collect(),
            );
            selection(options, request.selection.clone(), specs, package)
                .map_err(|error| HostError::Selection(error.into()))
        },
        |event| emit(event, &hidden.borrow()),
    )?)
}

fn selection(
    options: &[String],
    mut selection: PatchSelection,
    specs: &[&reseam_patcher::PatchSpec],
    package: Option<&str>,
) -> Result<PatchSelection> {
    if options.is_empty() {
        return Ok(selection);
    }
    let index = PatchIndex::new(specs)?;
    for raw in options {
        let invalid = || anyhow!("invalid option '{raw}': expected PATCH.KEY=VALUE");
        let (lhs, value) = raw.split_once('=').ok_or_else(invalid)?;
        let (patch_index, key) = option_target(&index, lhs, package)?;
        let spec = specs[patch_index];
        let patch = spec.reference();
        let declaration = spec
            .options
            .iter()
            .find(|declaration| declaration.key == key)
            .with_context(|| format!("unknown option '{key}' for patch '{patch}'"))?;
        let value =
            parse_option(declaration, value).with_context(|| format!("invalid --option {raw}"))?;
        selection
            .options
            .entry(patch)
            .or_default()
            .insert(key.to_owned(), value);
    }
    Ok(selection)
}

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
            Err(PatcherError::UnknownPatch(_)) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(anyhow!(
        "unknown patch in option '{lhs}'; expected PATCH.KEY (use <bundle>/<id>, an ID, or an unambiguous name)"
    ))
}

fn parse_option(declaration: &OptionDeclaration, raw: &str) -> Result<OptionValue> {
    let value = match declaration.option_type {
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
    };
    declaration.validate(&value)?;
    Ok(value)
}
