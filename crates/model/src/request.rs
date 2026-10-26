// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{OptionValue, PatchPreset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PatchRequest {
    pub apk_path: String,
    #[serde(default)]
    pub split_paths: Vec<String>,
    pub bundle_paths: Vec<String>,
    #[serde(default)]
    pub trust: Trust,
    #[serde(default)]
    pub selection: PatchSelection,
    pub output: PatchOutput,
    /// When absent, reuse or generate a pair next to the output.
    /// Supplied paths also generate on first use when both files are missing.
    /// Exactly one existing file is an error; existing credentials are never replaced.
    #[serde(default)]
    #[boltffi::default(None)]
    pub signing: Option<SigningKeyFiles>,
    #[serde(default)]
    #[boltffi::default(false)]
    pub dry_run: bool,
    #[serde(default)]
    #[boltffi::default(InstallMethod::Install)]
    pub install_method: InstallMethod,
    /// Writes the output without the patches that failed, and the patches that
    /// need them, instead of failing the run. The run undoes a failed patch, so
    /// the output is what it would have made had the patch not been selected.
    #[serde(default)]
    #[boltffi::default(false)]
    pub leave_out_failed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct SigningKeyFiles {
    pub key: String,
    pub cert: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum PatchOutput {
    /// Use `path` as a directory for splits, or append `.apk` for one component.
    Auto {
        path: String,
    },
    SingleFile {
        path: String,
    },
    SplitDir {
        path: String,
    },
}

impl PatchOutput {
    /// Requested destination. For automatic output, use the outcome for the final path.
    pub fn path(&self) -> &Path {
        match self {
            Self::Auto { path } | Self::SingleFile { path } | Self::SplitDir { path } => {
                Path::new(path)
            }
        }
    }
}

/// What the caller asked for: the patches `preset` selects for the APK's
/// package, plus `enable`, minus `disable`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(default)]
#[boltffi::data]
pub struct PatchSelection {
    #[boltffi::default(PatchPreset::Recommended)]
    pub preset: PatchPreset,
    pub enable: Vec<String>,
    pub disable: Vec<String>,
    pub options: HashMap<String, HashMap<String, OptionValue>>,
    /// Run patches on app versions they were not declared for. The package
    /// check still applies.
    #[boltffi::default(false)]
    pub ignore_versions: bool,
}

/// How the patched app reaches the device.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
#[boltffi::data]
pub enum InstallMethod {
    /// The output is installed as an app of its own.
    #[default]
    Install,
    /// The output replaces the files of the installed app in place. The system
    /// keeps the manifest it read from the installed app, so a mount build
    /// leaves out every patch that changes the manifest.
    Mount,
}

/// Ed25519 signer keys trusted by the caller, as 64-character hexadecimal strings.
/// Empty means no bundle is trusted. Validation happens before loading code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct Trust {
    #[serde(default)]
    pub keys: Vec<String>,
}
