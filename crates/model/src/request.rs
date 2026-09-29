// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{OptionValue, PatchPreset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
/// What the caller asked for: the patches `preset` selects for the APK's
/// package, plus `enable`, minus `disable`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

/// Ed25519 signer keys trusted by the caller, as 64-character hexadecimal strings.
/// Empty means no bundle is trusted. Validation happens before loading code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[boltffi::data]
pub struct Trust {
    #[serde(default)]
    pub keys: Vec<String>,
}
