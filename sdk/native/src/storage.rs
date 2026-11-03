// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The SDK's types as JSON, for hosts to store them and read published indexes.

use boltffi::export;
use reseam_model::{
    BuildRecord, PatchMetadata, PatchSelection, PublishedRelease, ReleaseChannel, ReleaseIndex,
    SdkError,
};
use serde::{Serialize, de::DeserializeOwned};

/// Encodes a selection for persistence in the SDK's serde schema.
#[export]
pub fn encode_selection(selection: PatchSelection) -> Result<String, SdkError> {
    to_json(&selection)
}

/// Restores a persisted selection, rejecting incompatible input.
#[export]
pub fn decode_selection(json: String) -> Result<PatchSelection, SdkError> {
    from_json(&json)
}

/// Encodes a build record for persistence in the SDK's serde schema.
#[export]
pub fn encode_build_record(record: BuildRecord) -> Result<String, SdkError> {
    to_json(&record)
}

/// Restores a persisted build record, rejecting incompatible input.
#[export]
pub fn decode_build_record(json: String) -> Result<BuildRecord, SdkError> {
    from_json(&json)
}

/// Reads a published release index, `patches.json` or `manager.json`.
#[export]
pub fn decode_release_index(json: String) -> Result<ReleaseIndex, SdkError> {
    from_json(&json)
}

/// The newest release of `index` on `channel`, or none when it has none.
#[export]
pub fn latest_release(index: ReleaseIndex, channel: ReleaseChannel) -> Option<PublishedRelease> {
    index.latest(channel).cloned()
}

/// Encodes patch metadata for persistence in the SDK's serde schema.
#[export]
pub fn encode_patch_metadata(patches: Vec<PatchMetadata>) -> Result<String, SdkError> {
    to_json(&patches)
}

/// Restores persisted patch metadata, rejecting incompatible input.
#[export]
pub fn decode_patch_metadata(json: String) -> Result<Vec<PatchMetadata>, SdkError> {
    from_json(&json)
}

fn to_json<T: Serialize>(value: &T) -> Result<String, SdkError> {
    serde_json::to_string(value).map_err(|error| SdkError::other(error.to_string()))
}

fn from_json<T: DeserializeOwned>(json: &str) -> Result<T, SdkError> {
    serde_json::from_str(json).map_err(|error| SdkError::other(error.to_string()))
}
