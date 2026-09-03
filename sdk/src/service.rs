// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::{Seek, Write};
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
                apk,
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
                apk,
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

    drop(aggregate_bundle);
    release_process_memory();

    Ok(PatchOutcome {
        results,
        artifact: Some(artifact),
        metrics: PatchMetrics::default(),
    })
}

/// Hands run-scoped memory back to the system so a long-lived host does not
/// carry one run's peak into the next: the runtime's garbage first, then the
/// native allocator's retained pages.
fn release_process_memory() {
    reseam_patcher::release_runtime_memory();
    purge_native_heap();
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn purge_native_heap() {
    // SAFETY: malloc_trim only releases free memory held by the allocator.
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(target_os = "android")]
fn purge_native_heap() {
    const M_PURGE: libc::c_int = -101;
    extern "C" {
        fn mallopt(param: libc::c_int, value: libc::c_int) -> libc::c_int;
    }
    // SAFETY: M_PURGE asks scudo to release cached free pages; it touches no live allocation.
    unsafe {
        mallopt(M_PURGE, 0);
    }
}

#[cfg(not(any(all(target_os = "linux", target_env = "gnu"), target_os = "android")))]
fn purge_native_heap() {}

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
    apk: ApkFile,
    output_path: &Path,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
    profiler: &mut PatchProfiler,
) -> Result<()> {
    let output_dir = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;

    let mut unsigned = profiler
        .measure(PatchPhase::WriteUnsignedArtifacts, || {
            apk.write_unsigned_files(ApkWriteOptions::default(), output_dir)
        })
        .context("failed to write patched APK")?;
    let (_, output) = unsigned.pop().context("no APK component was written")?;
    let signing_key = profiler.measure(PatchPhase::LoadSigningKey, || {
        load_or_generate_key(
            output_path.with_extension("pk8"),
            output_path.with_extension("der"),
            key_path,
            cert_path,
        )
    })?;
    profiler.measure(PatchPhase::SignArtifacts, || sign_into_place(&output, output_path, &signing_key))
}

fn write_signed_split_apks(
    apk: ApkFile,
    output_dir: &Path,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
    profiler: &mut PatchProfiler,
) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;

    let unsigned = profiler
        .measure(PatchPhase::WriteUnsignedArtifacts, || {
            apk.write_unsigned_files(ApkWriteOptions::default(), output_dir)
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
        for (name, output) in &unsigned {
            sign_into_place(output, &output_dir.join(name), &signing_key)?;
        }
        Ok(())
    })
}

/// Signs the unlinked output where it is and gives it its final name. The
/// file only appears at `output_path` complete and signed.
fn sign_into_place(output: &File, output_path: &Path, signing_key: &SigningKey) -> Result<()> {
    reseam_sign::v2::sign_file_in_place(output, signing_key).context("v2 signing failed")?;
    place_file(output, output_path)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    info!(output_path = %output_path.display(), "patched APK written");
    Ok(())
}

/// Links an unlinked temp file to `path`, falling back to a copy where the
/// file system cannot link anonymous files.
fn place_file(file: &File, path: &Path) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let source = std::ffi::CString::new(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
    let target = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())?;
    // SAFETY: both paths are valid C strings and linkat only creates a directory entry.
    let rc = unsafe {
        libc::linkat(libc::AT_FDCWD, source.as_ptr(), libc::AT_FDCWD, target.as_ptr(), libc::AT_SYMLINK_FOLLOW)
    };
    if rc == 0 {
        return Ok(());
    }
    let mut reader = std::io::BufReader::new(file);
    reader.seek(std::io::SeekFrom::Start(0))?;
    let mut writer = std::io::BufWriter::new(File::create(path)?);
    std::io::copy(&mut reader, &mut writer)?;
    writer.flush()
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
