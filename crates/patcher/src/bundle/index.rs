// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The patch index the Gradle plugin writes into a patch jar (`PatchIndex.kt`). Both sides check
//! `tests/fixtures/patch-index.json`.

pub(crate) const PATCH_INDEX: &str = "META-INF/reseam/patches.json";

#[cfg(feature = "bridge")]
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) enum MemberKind {
    #[serde(rename = "field")]
    Field,
    #[serde(rename = "method")]
    Method,
}

#[cfg(feature = "bridge")]
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct Declaration {
    #[serde(rename = "class")]
    pub(crate) class_name: String,
    pub(crate) owner: String,
    pub(crate) member: String,
    pub(crate) kind: MemberKind,
    pub(crate) id: String,
}
