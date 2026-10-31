// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use crate::error::{HostError, Result};

/// Dex metadata (`.dm`) for installing the base APK at `apk` on a device
/// running API level `sdk`, as [`reseam_apk::art_profile::dex_metadata`]
/// describes.
pub fn dex_metadata(apk: &Path, sdk: u32) -> Result<Option<Vec<u8>>> {
    reseam_apk::art_profile::dex_metadata(apk, sdk).map_err(|source| HostError::Apk {
        path: apk.to_owned(),
        source,
    })
}
