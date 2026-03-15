use crate::axml::string_pool::StringPool;
use crate::error::{ApkError, Result};

// Chunk types
const CHUNK_XML_DOCUMENT: u16 = 0x0003;
const CHUNK_STRING_POOL: u16 = 0x0001;
const CHUNK_RESOURCE_IDS: u16 = 0x0180;
const CHUNK_START_NAMESPACE: u16 = 0x0100;
const CHUNK_END_NAMESPACE: u16 = 0x0101;
const CHUNK_START_ELEMENT: u16 = 0x0102;
const CHUNK_END_ELEMENT: u16 = 0x0103;

// Well-known Android resource IDs for manifest attributes
// "package" is resolved by string name, not resource ID
const RES_VERSION_CODE: u32 = 0x0101_021b;
const RES_VERSION_NAME: u32 = 0x0101_021c;
const RES_MIN_SDK_VERSION: u32 = 0x0101_020c;
const RES_SPLIT: u32 = 0x0101_048a;

// Typed value data types
const TYPE_STRING: u8 = 0x03;
const TYPE_INT_DEC: u8 = 0x10;
const TYPE_INT_HEX: u8 = 0x11;
const TYPE_INT_BOOLEAN: u8 = 0x12;
const TYPE_REFERENCE: u8 = 0x01;

/// A parsed Android binary XML document.
#[derive(Debug, Clone)]
pub struct AxmlDocument {
    pub string_pool: StringPool,
    pub resource_ids: Vec<u32>,
    pub elements: Vec<AxmlEvent>,
}

/// Events in the AXML stream.
#[derive(Debug, Clone)]
pub enum AxmlEvent {
    StartNamespace {
        prefix: Option<u32>,
        uri: u32,
    },
    EndNamespace {
        prefix: Option<u32>,
        uri: u32,
    },
    StartElement {
        namespace: Option<u32>,
        name: u32,
        attributes: Vec<AxmlAttribute>,
    },
    EndElement {
        namespace: Option<u32>,
        name: u32,
    },
}

/// A single XML attribute.
#[derive(Debug, Clone)]
pub struct AxmlAttribute {
    pub namespace: Option<u32>,
    pub name: u32,
    pub raw_value: Option<u32>,
    pub typed_value: TypedValue,
}

/// Typed attribute value.
#[derive(Debug, Clone)]
pub enum TypedValue {
    String(u32),
    Int(i32),
    Bool(bool),
    Reference(u32),
    Hex(u32),
    Other { data_type: u8, data: u32 },
}

impl AxmlDocument {
    /// Parse a binary XML document from bytes (e.g., AndroidManifest.xml contents).
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            return Err(axml_err("AXML data too small"));
        }

        let chunk_type = read_u16(data, 0);
        if chunk_type != CHUNK_XML_DOCUMENT {
            return Err(axml_err(&format!(
                "Expected XML document chunk (0x0003), got 0x{:04x}",
                chunk_type
            )));
        }

        let header_size = read_u16(data, 2) as usize;
        let _total_size = read_u32(data, 4) as usize;

        let mut string_pool = None;
        let mut resource_ids = Vec::new();
        let mut elements = Vec::new();

        let mut pos = header_size;
        while pos + 8 <= data.len() {
            let ct = read_u16(data, pos);
            let hs = read_u16(data, pos + 2) as usize;
            let cs = read_u32(data, pos + 4) as usize;

            if cs < 8 || pos + cs > data.len() {
                break;
            }

            match ct {
                CHUNK_STRING_POOL => {
                    // chunk_data starts after the 8-byte header
                    let chunk_body = &data[pos + 8..pos + cs];
                    string_pool = Some(StringPool::parse(chunk_body, pos)?);
                }
                CHUNK_RESOURCE_IDS => {
                    let count = (cs - hs) / 4;
                    resource_ids = (0..count)
                        .map(|i| read_u32(data, pos + hs + i * 4))
                        .collect();
                }
                CHUNK_START_NAMESPACE => {
                    if pos + hs + 8 <= data.len() {
                        let prefix = optional_idx(read_u32(data, pos + hs));
                        let uri = read_u32(data, pos + hs + 4);
                        elements.push(AxmlEvent::StartNamespace { prefix, uri });
                    }
                }
                CHUNK_END_NAMESPACE => {
                    if pos + hs + 8 <= data.len() {
                        let prefix = optional_idx(read_u32(data, pos + hs));
                        let uri = read_u32(data, pos + hs + 4);
                        elements.push(AxmlEvent::EndNamespace { prefix, uri });
                    }
                }
                CHUNK_START_ELEMENT => {
                    if pos + hs + 20 <= data.len() {
                        let namespace = optional_idx(read_u32(data, pos + hs));
                        let name = read_u32(data, pos + hs + 4);
                        let _attr_start = read_u16(data, pos + hs + 8);
                        let attr_size = read_u16(data, pos + hs + 10) as usize;
                        let attr_count = read_u16(data, pos + hs + 12) as usize;

                        let attr_size = if attr_size == 0 { 20 } else { attr_size };
                        let attrs_offset = pos + hs + 16;

                        let mut attributes = Vec::with_capacity(attr_count);
                        for j in 0..attr_count {
                            let ao = attrs_offset + j * attr_size;
                            if ao + 20 > data.len() {
                                break;
                            }
                            let attr_ns = optional_idx(read_u32(data, ao));
                            let attr_name = read_u32(data, ao + 4);
                            let attr_raw = optional_idx(read_u32(data, ao + 8));
                            let _tv_size = read_u16(data, ao + 12);
                            let _tv_res0 = data[ao + 14];
                            let tv_type = data[ao + 15];
                            let tv_data = read_u32(data, ao + 16);

                            let typed_value = match tv_type {
                                TYPE_STRING => TypedValue::String(tv_data),
                                TYPE_INT_DEC => TypedValue::Int(tv_data as i32),
                                TYPE_INT_HEX => TypedValue::Hex(tv_data),
                                TYPE_INT_BOOLEAN => TypedValue::Bool(tv_data != 0),
                                TYPE_REFERENCE => TypedValue::Reference(tv_data),
                                _ => TypedValue::Other {
                                    data_type: tv_type,
                                    data: tv_data,
                                },
                            };

                            attributes.push(AxmlAttribute {
                                namespace: attr_ns,
                                name: attr_name,
                                raw_value: attr_raw,
                                typed_value,
                            });
                        }

                        elements.push(AxmlEvent::StartElement {
                            namespace,
                            name,
                            attributes,
                        });
                    }
                }
                CHUNK_END_ELEMENT => {
                    if pos + hs + 8 <= data.len() {
                        let namespace = optional_idx(read_u32(data, pos + hs));
                        let name = read_u32(data, pos + hs + 4);
                        elements.push(AxmlEvent::EndElement { namespace, name });
                    }
                }
                _ => {
                    // Unknown chunk, skip
                }
            }

            pos += cs;
        }

        let string_pool = string_pool.ok_or_else(|| axml_err("No string pool found in AXML"))?;

        Ok(AxmlDocument {
            string_pool,
            resource_ids,
            elements,
        })
    }

    /// Get a string from the pool by index.
    pub fn string(&self, index: u32) -> Option<&str> {
        self.string_pool.get(index)
    }

    /// Resolve the resource ID for a given string pool index (used for android:* attributes).
    pub fn resource_id_for(&self, string_idx: u32) -> Option<u32> {
        self.resource_ids.get(string_idx as usize).copied()
    }

    /// Extract the package name from a manifest document.
    pub fn package_name(&self) -> Option<&str> {
        self.find_root_attr_by_name("package")
            .and_then(|attr| self.attr_as_string(attr))
    }

    /// Extract versionCode from a manifest document.
    pub fn version_code(&self) -> Option<u32> {
        self.find_root_attr_by_res_id(RES_VERSION_CODE)
            .and_then(|attr| self.attr_as_int(attr))
    }

    /// Extract versionName from a manifest document.
    pub fn version_name(&self) -> Option<&str> {
        self.find_root_attr_by_res_id(RES_VERSION_NAME)
            .and_then(|attr| self.attr_as_string(attr))
    }

    /// Extract the split name (for split APKs). Returns None for base APKs.
    pub fn split_name(&self) -> Option<&str> {
        // Try resource ID first, then fallback to string name
        self.find_root_attr_by_res_id(RES_SPLIT)
            .or_else(|| self.find_root_attr_by_name("split"))
            .and_then(|attr| self.attr_as_string(attr))
    }

    /// Extract minSdkVersion from uses-sdk element.
    pub fn min_sdk_version(&self) -> Option<u32> {
        for event in &self.elements {
            if let AxmlEvent::StartElement {
                name, attributes, ..
            } = event
            {
                if self.string_pool.get(*name) == Some("uses-sdk") {
                    for attr in attributes {
                        if self.is_attr_res_id(attr, RES_MIN_SDK_VERSION) {
                            return self.attr_as_int(attr);
                        }
                    }
                }
            }
        }
        None
    }

    // --- Private helpers ---

    /// Find an attribute on the root (first) element by string name.
    fn find_root_attr_by_name(&self, attr_name: &str) -> Option<&AxmlAttribute> {
        if let Some(AxmlEvent::StartElement { attributes, .. }) = self
            .elements
            .iter()
            .find(|e| matches!(e, AxmlEvent::StartElement { .. }))
        {
            attributes
                .iter()
                .find(|a| self.string_pool.get(a.name) == Some(attr_name))
        } else {
            None
        }
    }

    /// Find an attribute on the root element by resource ID.
    fn find_root_attr_by_res_id(&self, res_id: u32) -> Option<&AxmlAttribute> {
        if let Some(AxmlEvent::StartElement { attributes, .. }) = self
            .elements
            .iter()
            .find(|e| matches!(e, AxmlEvent::StartElement { .. }))
        {
            attributes.iter().find(|a| self.is_attr_res_id(a, res_id))
        } else {
            None
        }
    }

    /// Check if an attribute's name string index maps to a given resource ID.
    fn is_attr_res_id(&self, attr: &AxmlAttribute, res_id: u32) -> bool {
        self.resource_id_for(attr.name) == Some(res_id)
    }

    /// Extract a string value from an attribute.
    fn attr_as_string(&self, attr: &AxmlAttribute) -> Option<&str> {
        // Try raw_value first (string pool index for the raw string representation)
        if let Some(raw) = attr.raw_value {
            if let Some(s) = self.string_pool.get(raw) {
                return Some(s);
            }
        }
        // Fall back to typed value
        if let TypedValue::String(idx) = attr.typed_value {
            return self.string_pool.get(idx);
        }
        None
    }

    /// Extract an integer value from an attribute.
    fn attr_as_int(&self, attr: &AxmlAttribute) -> Option<u32> {
        match attr.typed_value {
            TypedValue::Int(v) => Some(v as u32),
            TypedValue::Hex(v) => Some(v),
            _ => None,
        }
    }
}

fn optional_idx(value: u32) -> Option<u32> {
    if value == 0xFFFF_FFFF {
        None
    } else {
        Some(value)
    }
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn axml_err(reason: &str) -> ApkError {
    ApkError::AxmlError {
        reason: reason.to_string(),
    }
}
