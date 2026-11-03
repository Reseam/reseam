// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

/// The installed app a mount covers, as its device reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct InstalledApp {
    /// The long version code the package manager reports.
    pub version_code: i64,
    pub version_name: String,
    /// The base APK's file name in the app's folder.
    pub base_file: String,
    /// The splits it is installed with, by name, and the file holding each.
    pub splits: Vec<InstalledSplit>,
    /// The app's `.dm` file, when it has one. ART reads it before any other
    /// profile, so a mount has to cover it.
    #[serde(default)]
    #[boltffi::default(None)]
    pub dex_metadata_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct InstalledSplit {
    pub name: String,
    /// The file name in the app's folder.
    pub file: String,
}

/// Mounting a patched build over the installed app of `package_name`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct MountRequest {
    pub package_name: String,
    /// The app's name, for the root manager's module list.
    pub label: String,
    /// The patched build's version code, which has to be the installed one.
    pub version_code: i32,
    pub installed: InstalledApp,
    /// The patched base APK, at its path on the device.
    pub base: String,
    pub splits: Vec<PatchedSplit>,
    /// The patched base APK's dex metadata at its path on the device; give it
    /// when the installed app has a `.dm`.
    #[serde(default)]
    #[boltffi::default(None)]
    pub dex_metadata: Option<String>,
}

/// A patched split at its path on the device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PatchedSplit {
    pub name: String,
    pub path: String,
}

/// A script for a root shell (`su --mount-master -c sh`) and the mount
/// namespace it runs in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct RootScript {
    pub script: String,
    pub namespace: MountNamespace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
#[boltffi::data]
pub enum MountNamespace {
    /// The device's own, where apps see the mounts: run with `sh`.
    Global,
    /// A copy only the script sees: run with `unshare -m sh`.
    Private,
}
