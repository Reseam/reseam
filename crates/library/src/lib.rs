// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod inspect;
mod patching;
mod types;

pub use inspect::{built_in_trust_store, inspect_apk, inspect_with_trust, load_bundle_with_trust};
pub use patching::{build_execution_plan, parse_cli_option, patch, selection_from_cli};
pub use types::{
    ApkMetadata, ArtifactKind, BundleMetadata, CompatibilityMetadata, InputOptionValue,
    InspectResponse, OptionKind, OptionMetadata, PatchArtifact, PatchMetadata, PatchOutcome,
    PatchOutput, PatchRequest, PatchRunStatus, PatchSelection, RunEvent, TrustStatus, TrustStore,
};
