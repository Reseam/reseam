// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use crate::error::{HostError, Result};

/// Packs the split APKs `apks` into a new APKS file at `destination`, each
/// under its file name, as a run with split archive output writes them.
pub fn write_split_archive(apks: &[PathBuf], destination: &Path) -> Result<()> {
    let entries =
        apks.iter()
            .map(|apk| {
                let name = apk.file_name().and_then(|name| name.to_str()).ok_or(
                    HostError::InvalidRequest("split APK paths end in a file name"),
                )?;
                Ok((name, apk.as_path()))
            })
            .collect::<Result<Vec<_>>>()?;
    reseam_apk::write_split_archive(&entries, destination).map_err(|source| HostError::WriteApk {
        path: destination.to_owned(),
        source,
    })
}
