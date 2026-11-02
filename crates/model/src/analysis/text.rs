// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use super::refs::{CodeSite, XmlUse};
use super::show::ResourceId;

/// Where text a user sees comes from: resources, the XML and code that use
/// them, and DEX strings. Text a server sends matches nothing here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TextMatches {
    pub resources: Vec<StringResource>,
    pub dex_strings: Vec<DexString>,
}

/// A string resource whose value contains the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct StringResource {
    pub resource: ResourceId,
    /// The matching value, from the default configuration when it matches there.
    pub value: String,
    /// Every configuration whose value contains the text.
    pub qualifiers: Vec<String>,
    /// Code that loads the resource's id.
    pub loaded_by: Vec<CodeSite>,
    pub xml: Vec<XmlHit>,
}

/// An XML element that uses a matching resource, and the code that reaches it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct XmlHit {
    pub usage: XmlUse,
    /// Code that loads the file's resource id, as inflating a layout does.
    pub file_loaded_by: Vec<CodeSite>,
    /// Code that loads the element's view id, as finding the view does.
    pub view_loaded_by: Vec<CodeSite>,
}

/// A DEX string constant containing the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DexString {
    pub value: String,
    pub loaded_by: Vec<CodeSite>,
}
