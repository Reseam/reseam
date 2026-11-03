// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::{
    AppIdentity, Incompatibility, InstallMethod, OptionProblem, PatchMetadata, PatchPreset,
    PatchRef, PatchSelection,
};

/// What a run of `selection` over `patches` would do for `app`, without
/// opening the APK or loading patch code.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PlanRequest {
    /// The patches an inspection lists.
    pub patches: Vec<PatchMetadata>,
    pub app: AppIdentity,
    #[serde(default)]
    pub selection: PatchSelection,
    #[serde(default)]
    #[boltffi::default(InstallMethod::Install)]
    pub install_method: InstallMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct SelectionPlan {
    /// The patches users see for the app, those that declare its package
    /// and universal ones, then the hidden patches the run needs, in request
    /// order.
    pub patches: Vec<PlannedPatch>,
    /// Selection entries that name no patch, such as patches a later bundle
    /// release removed. The plan leaves them out; a run fails on them.
    pub unknown: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PlannedPatch {
    pub patch: PatchRef,
    /// Why the patch does not apply to the app.
    #[boltffi::default(None)]
    pub incompatibility: Option<Incompatibility>,
    /// The presets that select the patch for the app.
    pub presets: Vec<PatchPreset>,
    /// The selection asks for the patch, as opposed to a patch needing it.
    pub selected: bool,
    pub run: PlannedRun,
    /// The selected patches that need this one, empty when it is selected.
    pub required_by: Vec<PatchRef>,
    /// What is wrong with the option values the patch would run with.
    pub option_problems: Vec<OptionIssue>,
}

/// Whether the run would execute a patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum PlannedRun {
    /// Neither selected nor needed.
    Off,
    Runs,
    /// Covered, but left out before running, for `reason`.
    Skipped {
        reason: String,
    },
    /// Covered, but left out of a mount build.
    Unmountable {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct OptionIssue {
    pub key: String,
    pub problem: OptionProblem,
}
