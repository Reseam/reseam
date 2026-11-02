// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::PatchRef;

/// The Kotlin compiler a host provides: its jars and their dependencies, and
/// the full standard library to compile against. The patch runtime carries a
/// shrunk standard library, enough to run patches but not to compile them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct KotlinCompiler {
    pub jars: Vec<String>,
    pub stdlib: String,
}

/// Kotlin to run against an APK, inside a generated file that imports the patch API's packages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", content = "text", rename_all = "snake_case")]
pub enum Snippet {
    /// An expression whose value is a target, such as `method { returns(Type.Boolean) }`.
    Target(String),
    /// Statements run with the patch runtime as receiver; what they print is the result.
    Code(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", content = "result", rename_all = "snake_case")]
pub enum Inspected {
    Target(TargetInspection),
    Output(String),
}

/// What a target matched: every candidate its query accepted, the one
/// resolution picks, and the closest misses when none was accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TargetInspection {
    /// Descriptors of the accepted candidates, best ranked first.
    pub matches: Vec<String>,
    /// `None` when nothing matched, or a query for one item matched several it cannot rank.
    pub winner: Option<String>,
    /// Whether the target resolves to every match, as `methods { }` does, rather than to one.
    pub many: bool,
    pub considered: u32,
    pub reasons: Vec<String>,
    pub near_misses: Vec<String>,
    /// Why resolving failed, for a target that is not a query or whose query
    /// the patch API rejected.
    pub failure: Option<String>,
}

/// How one target resolved while a patch ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TargetResolution {
    /// The patch that resolved it; `None` outside a patch.
    pub patch: Option<PatchRef>,
    /// The target's name, or a description when it has none.
    pub target: String,
    /// Its failure, when there is one, failed the patch.
    pub inspection: TargetInspection,
}

/// A compiler error, at a line and column of the snippet as given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CompileError {
    pub line: u32,
    pub column: u32,
    pub message: String,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}
