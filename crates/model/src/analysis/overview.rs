// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use super::show::ApkView;
use super::symbol::ClassRef;

/// What runs an app, where its code is and where to start reading it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Overview {
    pub apk: ApkView,
    /// Code outside the DEX that holds app logic. Empty when the DEX is all there is.
    pub runtimes: Vec<Runtime>,
    pub dex_language: DexLanguage,
    /// Signs of protections seen in real APKs before. No marker does not mean unprotected.
    pub markers: Vec<Marker>,
    pub code: Vec<PackageGroup>,
    pub sdks: Vec<SdkUse>,
    pub entry: EntryPoints,
}

/// Code outside the DEX, and the files that show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Runtime {
    pub kind: RuntimeKind,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeKind {
    /// Compiled Dart in `libapp.so`.
    Flutter,
    /// A React Native bundle compiled to Hermes bytecode.
    Hermes { bytecode_version: u32 },
    /// A React Native bundle of plain JavaScript.
    JavaScript,
    /// C# compiled to native code by IL2CPP.
    Unity,
    /// .NET assemblies run by Mono (Xamarin, MAUI).
    DotNet,
}

/// Which language the DEX was compiled from, by the runtime library it carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DexLanguage {
    Kotlin,
    Java,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Marker {
    pub name: String,
    pub evidence: Vec<String>,
}

/// Classes under one package prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PackageGroup {
    pub group: CodeGroup,
    pub classes: usize,
    /// Classes whose own name is at most three characters, as R8 names them.
    pub obfuscated: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", content = "package", rename_all = "snake_case")]
pub enum CodeGroup {
    /// The app's own package.
    App(String),
    /// A library the engine knows by its package.
    Library(String),
    /// Any other top-level package.
    Other(String),
    /// Top-level packages of one to three characters, where R8 moves renamed classes.
    ShortPackages,
}

/// An SDK found by its package, or by manifest entries when its names were not kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SdkUse {
    pub category: SdkCategory,
    pub name: String,
    /// The package its API lives in, in Java form.
    pub package: String,
    /// Classes found under the package; 0 when only the manifest shows the SDK.
    pub classes: usize,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SdkCategory {
    Billing,
    Ads,
    SignIn,
    Analytics,
    CrashReporting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct EntryPoints {
    pub application: Option<ClassRef>,
    pub launcher: Option<ClassRef>,
    pub activities: usize,
    pub services: usize,
    pub receivers: usize,
    pub providers: usize,
}
