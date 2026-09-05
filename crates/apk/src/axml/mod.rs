// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Android binary XML: parsing, editing, serializing, and compiling from text.

pub mod android_attrs;
mod compiler;
mod document;
mod edit;
mod manifest;
mod reader;
mod writer;

pub use android_attrs::android_attr_res_id;
pub use compiler::{
    build_document, compile_xml, is_compiled_axml, parse_attribute_value, AttributeValue,
};
pub use document::{AxmlAttribute, AxmlDocument, AxmlEvent};

pub const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";
pub const APP_NS: &str = "http://schemas.android.com/apk/res-auto";

const CHUNK_XML_DOCUMENT: u16 = 0x0003;
const CHUNK_RESOURCE_IDS: u16 = 0x0180;
const CHUNK_START_NAMESPACE: u16 = 0x0100;
const CHUNK_END_NAMESPACE: u16 = 0x0101;
const CHUNK_START_ELEMENT: u16 = 0x0102;
const CHUNK_END_ELEMENT: u16 = 0x0103;
const NONE: u32 = 0xFFFF_FFFF;
