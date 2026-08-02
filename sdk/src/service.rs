// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use reseam_apk::{ApkFile, ApkWriteOptions};
use reseam_patcher::bundle::{BundleKeepAlive, PatchBundle};
use reseam_patcher::context::PatchContext;
use reseam_patcher::engine::{self, PatchStatus, ProgressEvent};
use tracing::info;

use reseam_sign::{GeneratedKey, SigningKey};

use crate::dto::{
    ArtifactKind, PatchArtifact, PatchOutcome, PatchOutput, PatchRequest, PatchRunStatus, RunEvent,
};
use crate::inspect::{load_bundle_with_trust, open_patch_apk};
use crate::metrics::{
    ApplyDiagnostics, PatchExecutionReport, PatchMetrics, PatchPhase, PatchProfiler,
};
use crate::selection::compile_patch_selection;

pub fn patch<F>(request: &PatchRequest, mut emit: F) -> Result<PatchOutcome>
where
    F: FnMut(RunEvent),
{
    measure_patch(request, &mut emit).outcome
}

pub fn measure_patch<F>(request: &PatchRequest, mut emit: F) -> PatchExecutionReport
where
    F: FnMut(RunEvent),
{
    let mut profiler = PatchProfiler::new();
    let outcome = patch_with_profiler(request, &mut emit, &mut profiler);
    let metrics = profiler.finish();

    let outcome = outcome.map(|mut outcome| {
        outcome.metrics = metrics.clone();
        outcome
    });

    PatchExecutionReport { outcome, metrics }
}

fn patch_with_profiler<F>(
    request: &PatchRequest,
    emit: &mut F,
    profiler: &mut PatchProfiler,
) -> Result<PatchOutcome>
where
    F: FnMut(RunEvent),
{
    emit(RunEvent::Info {
        message: format!("Opening APK {}", request.apk_path.display()),
    });
    let mut apk = profiler.measure(PatchPhase::OpenApk, || {
        open_patch_apk(&request.apk_path, &request.split_paths)
    })?;

    if request.bundle_paths.is_empty() {
        bail!("at least one bundle is required");
    }

    let loaded_bundles = profiler.measure(PatchPhase::LoadBundles, || -> Result<_> {
        let mut loaded_bundles = Vec::with_capacity(request.bundle_paths.len());
        for bundle_path in &request.bundle_paths {
            emit(RunEvent::Info {
                message: format!("Loading bundle {}", bundle_path.display()),
            });
            loaded_bundles.push(load_bundle_with_trust(bundle_path, &request.trust_store)?);
        }
        Ok(loaded_bundles)
    })?;

    let aggregate_bundle = aggregate_bundles(loaded_bundles);
    let selection = profiler.measure(PatchPhase::CompileSelection, || {
        compile_patch_selection(&aggregate_bundle.patches, &request.selection)
    })?;

    if request.dry_run {
        let results = profiler
            .measure(PatchPhase::ValidatePatches, || {
                engine::validate_patches_with_selection(
                    &aggregate_bundle.patches,
                    &selection,
                    apk.package_name(),
                    apk.version_name(),
                )
            })
            .context("patch validation failed")?;
        for result in &results {
            emit(status_event(result));
        }
        return Ok(PatchOutcome {
            results,
            artifact: None,
            metrics: PatchMetrics::default(),
        });
    }

    let mut ctx = PatchContext::new(&mut apk);
    let results = profiler
        .measure(PatchPhase::ApplyPatches, || {
            engine::apply_patches_with_selection_and_observer(
                &mut ctx,
                &aggregate_bundle.patches,
                &selection,
                |event| match event {
                    ProgressEvent::PatchStarted { patch } => emit(RunEvent::PatchStarted { patch }),
                    ProgressEvent::PatchLog(log) => emit(RunEvent::PatchLog {
                        patch: log.patch,
                        level: log.level.to_string(),
                        message: log.message,
                    }),
                    ProgressEvent::PatchFinished { patch, status } => {
                        emit(RunEvent::PatchFinished {
                            patch,
                            reason: patch_status_reason(&status),
                            status: patch_run_status(&status),
                        })
                    }
                },
            )
        })
        .context("patch application failed")?;
    profiler.set_apply_diagnostics(capture_apply_diagnostics(&ctx));
    drop(ctx);

    let failures = results
        .iter()
        .filter_map(|result| match &result.status {
            PatchStatus::Failed { reason } => Some(format!(
                "{}: {}",
                result.name,
                concise_failure_reason(reason)
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        bail!(
            "{} patch(es) failed:\n{}",
            failures.len(),
            failures.join("\n")
        );
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
                profiler,
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
                profiler,
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
        metrics: PatchMetrics::default(),
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

fn concise_failure_reason(reason: &str) -> String {
    let mut lines = reason
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("at "));
    let first = lines.next().unwrap_or(reason).to_string();
    let details =
        lines.find(|line| line.starts_with("Reasons:") || line.starts_with("Near misses:"));
    match details {
        Some(details) => format!("{first}; {details}"),
        None => first,
    }
}

fn capture_apply_diagnostics(ctx: &PatchContext) -> ApplyDiagnostics {
    let breakdown = ctx.apk().dex().memory_breakdown();
    let stats = breakdown.materialized;
    let jvm = reseam_patcher::jvm_heap_stats();
    ApplyDiagnostics {
        rss_bytes: PatchProfiler::current_rss_bytes(),
        total_classes: stats.total_classes,
        resolved_classes: stats.resolved_classes,
        materialized_methods: stats.methods,
        materialized_instructions: stats.instructions,
        estimated_ir_bytes: stats.estimated_ir_bytes(),
        raw_buffer_bytes: breakdown.raw_buffer_bytes,
        string_pool_bytes: breakdown.string_pool_bytes,
        string_count: breakdown.string_count,
        id_table_bytes: breakdown.id_table_bytes,
        class_def_bytes: breakdown.class_def_bytes,
        jvm_used_bytes: jvm.map(|j| j.used_bytes),
        jvm_committed_bytes: jvm.map(|j| j.committed_bytes),
        jvm_max_bytes: jvm.map(|j| j.max_bytes),
    }
}

fn write_signed_single_apk(
    apk: &mut ApkFile,
    output_path: &Path,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
    profiler: &mut PatchProfiler,
) -> Result<()> {
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }

    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    profiler
        .measure(PatchPhase::WriteUnsignedArtifacts, || {
            apk.write_to_with_options(
                tmp_dir.path(),
                ApkWriteOptions {
                    strip_signatures: true,
                },
            )
        })
        .context("failed to write patched APK")?;

    let tmp_apk_path = find_output_apks(tmp_dir.path())?
        .into_iter()
        .next()
        .context("no APK file found in output directory")?;
    let signing_key = profiler.measure(PatchPhase::LoadSigningKey, || {
        load_or_generate_key(
            output_path.with_extension("pk8"),
            output_path.with_extension("der"),
            key_path,
            cert_path,
        )
    })?;
    profiler.measure(PatchPhase::SignArtifacts, || {
        sign_apk_to_path(&tmp_apk_path, output_path, &signing_key)
    })
}

fn write_signed_split_apks(
    apk: &mut ApkFile,
    output_dir: &Path,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
    profiler: &mut PatchProfiler,
) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;

    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    profiler
        .measure(PatchPhase::WriteUnsignedArtifacts, || {
            apk.write_to_with_options(
                tmp_dir.path(),
                ApkWriteOptions {
                    strip_signatures: true,
                },
            )
        })
        .context("failed to write patched APK set")?;

    let signing_key = profiler.measure(PatchPhase::LoadSigningKey, || {
        load_or_generate_key(
            output_dir.join("reseam.pk8"),
            output_dir.join("reseam.der"),
            key_path,
            cert_path,
        )
    })?;

    profiler.measure(PatchPhase::SignArtifacts, || -> Result<()> {
        for unsigned_apk in find_output_apks(tmp_dir.path())? {
            let file_name = unsigned_apk
                .file_name()
                .context("temporary APK output is missing a filename")?;
            let output_path = output_dir.join(file_name);
            sign_apk_to_path(&unsigned_apk, &output_path, &signing_key)?;
        }

        Ok(())
    })?;

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

fn sign_apk_to_path(
    unsigned_path: &Path,
    output_path: &Path,
    signing_key: &SigningKey,
) -> Result<()> {
    let output_file = std::fs::File::create(output_path)
        .with_context(|| format!("failed to create {}", output_path.display()))?;
    let mut output = std::io::BufWriter::new(output_file);
    sign_apk_file(unsigned_path, signing_key, &mut output)?;
    use std::io::Write as _;
    output
        .flush()
        .with_context(|| format!("failed to flush {}", output_path.display()))?;
    info!(output_path = %output_path.display(), "patched APK written");
    Ok(())
}

fn sign_apk_file(
    unsigned_path: &Path,
    signing_key: &SigningKey,
    output: &mut dyn std::io::Write,
) -> Result<()> {
    let file = std::fs::File::open(unsigned_path)
        .with_context(|| format!("failed to open {}", unsigned_path.display()))?;
    // SAFETY: The input file is treated as immutable for the duration of the mapping.
    let mmap = unsafe { memmap2::Mmap::map(&file) };

    match mmap {
        Ok(mapped) => reseam_sign::v2::sign_to_writer(&mapped, signing_key, output)
            .context("v2 signing failed"),
        Err(_) => {
            let unsigned_bytes = std::fs::read(unsigned_path)
                .with_context(|| format!("failed to read {}", unsigned_path.display()))?;
            reseam_sign::v2::sign_to_writer(&unsigned_bytes, signing_key, output)
                .context("v2 signing failed")
        }
    }
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

    SigningKey::from_files(&key_path, &cert_path).context("failed to load signing key")
}
