// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use boltffi::export;
#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject};
use reseam_patcher::engine::{PatchResult, PatchStatus};
use serde::{Deserialize, Serialize};

#[export]
pub trait PatchEventSink {
    fn on_event(&self, event_json: String);
}

#[derive(Debug, Deserialize)]
struct InspectRequest {
    #[serde(default)]
    apk_path: Option<PathBuf>,
    #[serde(default)]
    split_paths: Vec<PathBuf>,
    bundle_paths: Vec<PathBuf>,
    #[serde(default = "default_include_builtin_trust")]
    include_builtin_trust: bool,
    #[serde(default)]
    trusted_public_keys_hex: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PatchRequestJson {
    apk_path: PathBuf,
    #[serde(default)]
    split_paths: Vec<PathBuf>,
    bundle_paths: Vec<PathBuf>,
    output: PatchOutputJson,
    #[serde(default)]
    selection: PatchSelection,
    #[serde(default = "default_include_builtin_trust")]
    include_builtin_trust: bool,
    #[serde(default)]
    trusted_public_keys_hex: Vec<String>,
    #[serde(default)]
    key_path: Option<PathBuf>,
    #[serde(default)]
    cert_path: Option<PathBuf>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PatchOutputJson {
    SingleFile { path: PathBuf },
    SplitDir { path: PathBuf },
}

#[derive(Debug, Serialize)]
struct FfiPatchOutcome {
    results: Vec<FfiPatchResult>,
    artifact: Option<PatchArtifact>,
    metrics: PatchMetrics,
}

#[derive(Debug, Serialize)]
struct FfiPatchResult {
    name: String,
    status: FfiPatchStatus,
    logs: Vec<FfiLogEntry>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FfiPatchStatus {
    Applied,
    Skipped { reason: String },
    Failed { reason: String },
}

#[derive(Debug, Serialize)]
struct FfiLogEntry {
    level: String,
    patch: String,
    message: String,
}

#[export]
pub fn inspect_apk_json(apk_path: String, split_paths_json: String) -> Result<String, String> {
    let split_paths: Vec<PathBuf> = serde_json::from_str(&split_paths_json).map_err(json_error)?;
    let metadata =
        inspect_apk(PathBuf::from(apk_path).as_path(), &split_paths).map_err(format_error)?;
    serde_json::to_string(&metadata).map_err(json_error)
}

#[export]
pub fn inspect_json(request_json: String) -> Result<String, String> {
    let request: InspectRequest = serde_json::from_str(&request_json).map_err(json_error)?;
    let trust_store = trust_store(
        request.include_builtin_trust,
        &request.trusted_public_keys_hex,
    )?;
    let response = inspect_with_trust(
        &request.bundle_paths,
        request.apk_path.as_deref(),
        &request.split_paths,
        &trust_store,
    )
    .map_err(format_error)?;
    serde_json::to_string(&response).map_err(json_error)
}

#[export]
pub fn patch_json(request_json: String, event_sink: impl PatchEventSink) -> Result<String, String> {
    let request = patch_request_from_json(request_json)?;
    let report = measure_patch(&request, |event| emit_event(&event_sink, event));
    match report.outcome {
        Ok(outcome) => serde_json::to_string(&FfiPatchOutcome {
            results: outcome.results.iter().map(ffi_patch_result).collect(),
            artifact: outcome.artifact,
            metrics: outcome.metrics,
        })
        .map_err(json_error),
        Err(error) => Err(format_error(error)),
    }
}

fn format_error(error: impl std::fmt::Display) -> String {
    format!("{error:#}")
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_app_reseam_sdk_ReseamAndroidHost_setClassLoader(
    mut env: jni::JNIEnv<'_>,
    _class: JClass<'_>,
    loader: JObject<'_>,
) {
    if let Err(error) = reseam_patcher::kotlin::android_host::install_class_loader(&mut env, loader)
    {
        let _ = env.throw_new("java/lang/IllegalStateException", error);
    }
}

fn patch_request_from_json(request_json: String) -> Result<PatchRequest, String> {
    let request: PatchRequestJson = serde_json::from_str(&request_json).map_err(json_error)?;
    let trust_store = trust_store(
        request.include_builtin_trust,
        &request.trusted_public_keys_hex,
    )?;
    Ok(PatchRequest {
        apk_path: request.apk_path,
        split_paths: request.split_paths,
        bundle_paths: request.bundle_paths,
        trust_store,
        selection: request.selection,
        output: match request.output {
            PatchOutputJson::SingleFile { path } => PatchOutput::SingleFile(path),
            PatchOutputJson::SplitDir { path } => PatchOutput::SplitDir(path),
        },
        key_path: request.key_path,
        cert_path: request.cert_path,
        dry_run: request.dry_run,
    })
}

fn trust_store(include_builtin: bool, extra_keys_hex: &[String]) -> Result<TrustStore, String> {
    let mut keys = if include_builtin {
        built_in_trust_store().keys().to_vec()
    } else {
        Vec::new()
    };

    for key_hex in extra_keys_hex {
        let bytes = hex::decode(key_hex)
            .map_err(|error| format!("invalid trusted public key hex `{key_hex}`: {error}"))?;
        let key: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
            format!(
                "trusted public key `{key_hex}` has {} bytes; expected 32",
                bytes.len()
            )
        })?;
        keys.push(key);
    }

    Ok(TrustStore::new(keys))
}

fn emit_event(event_sink: &impl PatchEventSink, event: RunEvent) {
    let event_json = serde_json::to_string(&event).unwrap_or_else(|error| {
        format!(r#"{{"type":"info","message":"event encode failed: {error}"}}"#)
    });
    event_sink.on_event(event_json);
}

fn ffi_patch_result(result: &PatchResult) -> FfiPatchResult {
    FfiPatchResult {
        name: result.name.clone(),
        status: match &result.status {
            PatchStatus::Applied => FfiPatchStatus::Applied,
            PatchStatus::Skipped { reason } => FfiPatchStatus::Skipped {
                reason: reason.clone(),
            },
            PatchStatus::Failed { reason } => FfiPatchStatus::Failed {
                reason: reason.clone(),
            },
        },
        logs: result
            .logs
            .iter()
            .map(|log| FfiLogEntry {
                level: log.level.to_string(),
                patch: log.patch.clone(),
                message: log.message.clone(),
            })
            .collect(),
    }
}

fn json_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn default_include_builtin_trust() -> bool {
    true
}
