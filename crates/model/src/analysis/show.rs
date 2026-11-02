// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use super::refs::CodeSite;
use super::symbol::{ClassRef, FieldRef, Member, MethodRef};

/// The APK as a file: who it is, how it is split, how it is signed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApkView {
    pub package: Option<String>,
    pub version_name: Option<String>,
    pub version_code: Option<i32>,
    pub min_sdk: Option<u32>,
    /// Split names after the base APK, empty for a single APK.
    pub splits: Vec<String>,
    pub dex_files: usize,
    pub classes: usize,
    pub methods: usize,
    /// SHA-256 of each signing certificate, lowercase hex.
    pub signers: Vec<String>,
}

/// A name the DEX still holds for an obfuscated class or method.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NameHint {
    pub name: String,
    pub source: HintSource,
}

/// A class or method a name hint names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NamedMember {
    pub member: Member,
    pub hint: NameHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum HintSource {
    /// The class's `toString` starts its text with the name, as data classes do.
    ToString,
    /// The `SourceFile` attribute.
    SourceFile,
    /// The tag the class logs with.
    LogTag,
    /// A parameter name from Kotlin's null check on it.
    ParameterCheck,
}

/// One method's body as smali.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MethodView {
    pub method: MethodRef,
    /// Access flags as smali names them: `public static final`.
    pub access: String,
    pub registers: u16,
    pub parameter_registers: u16,
    pub hints: Vec<NameHint>,
    /// Empty for abstract and native methods.
    pub lines: Vec<BodyLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BodyLine {
    pub kind: LineKind,
    /// The instruction's index, for instruction lines.
    pub index: Option<u32>,
    pub text: String,
    /// The class, method or field the instruction names.
    pub refers: Option<Member>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Label,
    Instruction,
    Directive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ClassView {
    pub class: ClassRef,
    pub access: String,
    pub superclass: Option<ClassRef>,
    pub interfaces: Vec<ClassRef>,
    pub source_file: Option<String>,
    pub hints: Vec<NameHint>,
    pub fields: Vec<FieldView>,
    pub methods: Vec<MethodSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FieldView {
    pub field: FieldRef,
    pub access: String,
    /// A static field's initial value as smali writes it.
    pub initial: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MethodSummary {
    pub method: MethodRef,
    pub access: String,
    pub hints: Vec<NameHint>,
}

/// An activity, service, receiver or provider, with the code around it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ComponentView {
    pub kind: ComponentKind,
    pub class: ClassRef,
    /// The manifest element, as XML.
    pub manifest: String,
    /// Layouts the component's class loads by id.
    pub layouts: Vec<ResourceId>,
    /// Code that names the component's class, as starting it takes.
    pub started_by: Vec<CodeSite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    Activity,
    ActivityAlias,
    Service,
    Receiver,
    Provider,
}

/// A resource id and the name the table gives it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ResourceId {
    pub id: u32,
    /// `@string/title`, or `None` when no configuration names it.
    pub name: Option<String>,
}

/// A resource's value in each configuration that defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ResourceView {
    pub resource: ResourceId,
    pub values: Vec<ConfigValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ConfigValue {
    /// Empty for the default configuration.
    pub qualifiers: String,
    /// The value as resource XML writes it; a style or array lists its items.
    pub value: String,
}
