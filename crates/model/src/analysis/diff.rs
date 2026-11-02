// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

/// Two versions of one body, aligned line by line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BodyDiff {
    pub lines: Vec<DiffLine>,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DiffLine {
    pub change: Change,
    /// The instruction index on the left, for instruction lines it has.
    pub left: Option<u32>,
    pub right: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Same,
    Added,
    Removed,
}
