// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Android binary XML: parsing, editing, serializing, and compiling from text.

pub mod android_attrs;
mod compiler;
mod document;
mod edit;
mod inline;
mod manifest;
mod reader;
mod text;
mod writer;

pub use android_attrs::{android_attr_res_id, android_attr_symbol, android_res_id};
pub use compiler::{
    AttributeValue, attribute_symbols, build_document, compile_xml, infer_value, is_compiled_axml,
    parse_attribute_value,
};
pub use document::{AxmlAttribute, AxmlDocument, AxmlEvent, NodeMetadata};
pub use inline::{AAPT_NS, CompiledXmlResource, compile_resource_file};

pub const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";
pub const APP_NS: &str = "http://schemas.android.com/apk/res-auto";
/// Build-time hints. aapt strips them, and so does the compiler.
pub const TOOLS_NS: &str = "http://schemas.android.com/tools";

const CHUNK_XML_DOCUMENT: u16 = 0x0003;
const CHUNK_RESOURCE_IDS: u16 = 0x0180;
const CHUNK_START_NAMESPACE: u16 = 0x0100;
const CHUNK_END_NAMESPACE: u16 = 0x0101;
const CHUNK_START_ELEMENT: u16 = 0x0102;
const CHUNK_END_ELEMENT: u16 = 0x0103;
const CHUNK_TEXT: u16 = 0x0104;
const NONE: u32 = 0xFFFF_FFFF;
