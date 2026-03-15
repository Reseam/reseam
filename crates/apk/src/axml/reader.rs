use crate::axml::string_pool::StringPool;
use crate::error::{invalid, malformed, read_u16_le, read_u32_le, read_u8, require_len, Result};

const CHUNK_XML_DOCUMENT: u16 = 0x0003;
const CHUNK_STRING_POOL: u16 = 0x0001;
const CHUNK_RESOURCE_IDS: u16 = 0x0180;
const CHUNK_START_NAMESPACE: u16 = 0x0100;
const CHUNK_END_NAMESPACE: u16 = 0x0101;
const CHUNK_START_ELEMENT: u16 = 0x0102;
const CHUNK_END_ELEMENT: u16 = 0x0103;

const RES_VERSION_CODE: u32 = 0x0101_021b;
const RES_VERSION_NAME: u32 = 0x0101_021c;
const RES_MIN_SDK_VERSION: u32 = 0x0101_020c;
const RES_SPLIT: u32 = 0x0101_048a;

const TYPE_STRING: u8 = 0x03;
const TYPE_INT_DEC: u8 = 0x10;
const TYPE_INT_HEX: u8 = 0x11;
const TYPE_INT_BOOLEAN: u8 = 0x12;
const TYPE_REFERENCE: u8 = 0x01;

#[derive(Debug, Clone)]
pub struct AxmlDocument {
    pub string_pool: StringPool,
    pub resource_ids: Vec<u32>,
    pub elements: Vec<AxmlEvent>,
}

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

#[derive(Debug, Clone)]
pub struct AxmlAttribute {
    pub namespace: Option<u32>,
    pub name: u32,
    pub raw_value: Option<u32>,
    pub typed_value: TypedValue,
}

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
    pub fn parse(data: &[u8]) -> Result<Self> {
        require_len(data, 0, 8, "axml document")?;

        let chunk_type = read_u16_le(data, 0, "axml document")?;
        if chunk_type != CHUNK_XML_DOCUMENT {
            return Err(invalid(
                "axml document",
                format!("expected XML document chunk (0x0003), got 0x{chunk_type:04x}"),
            ));
        }

        let header_size = read_u16_le(data, 2, "axml document")? as usize;
        let _total_size = read_u32_le(data, 4, "axml document")? as usize;

        let mut string_pool = None;
        let mut resource_ids = Vec::new();
        let mut elements = Vec::new();

        let mut pos = header_size;
        while pos + 8 <= data.len() {
            let ct = read_u16_le(data, pos, "axml chunk")?;
            let hs = read_u16_le(data, pos + 2, "axml chunk")? as usize;
            let cs = read_u32_le(data, pos + 4, "axml chunk")? as usize;

            if cs < 8 || pos + cs > data.len() {
                return Err(malformed(
                    "axml chunk",
                    pos,
                    "chunk extends past end of document",
                ));
            }

            match ct {
                CHUNK_STRING_POOL => {
                    let chunk_body = &data[pos + 8..pos + cs];
                    string_pool = Some(StringPool::parse(chunk_body, pos)?);
                }
                CHUNK_RESOURCE_IDS => {
                    let count = (cs.saturating_sub(hs)) / 4;
                    let mut ids = Vec::with_capacity(count);
                    for i in 0..count {
                        ids.push(read_u32_le(data, pos + hs + i * 4, "axml resource ids")?);
                    }
                    resource_ids = ids;
                }
                CHUNK_START_NAMESPACE => {
                    require_len(data, pos + hs, 8, "axml namespace")?;
                    let prefix = optional_idx(read_u32_le(data, pos + hs, "axml namespace")?);
                    let uri = read_u32_le(data, pos + hs + 4, "axml namespace")?;
                    elements.push(AxmlEvent::StartNamespace { prefix, uri });
                }
                CHUNK_END_NAMESPACE => {
                    require_len(data, pos + hs, 8, "axml namespace")?;
                    let prefix = optional_idx(read_u32_le(data, pos + hs, "axml namespace")?);
                    let uri = read_u32_le(data, pos + hs + 4, "axml namespace")?;
                    elements.push(AxmlEvent::EndNamespace { prefix, uri });
                }
                CHUNK_START_ELEMENT => {
                    require_len(data, pos + hs, 20, "axml start element")?;
                    let namespace =
                        optional_idx(read_u32_le(data, pos + hs, "axml start element")?);
                    let name = read_u32_le(data, pos + hs + 4, "axml start element")?;
                    let _attr_start = read_u16_le(data, pos + hs + 8, "axml start element")?;
                    let attr_size =
                        read_u16_le(data, pos + hs + 10, "axml start element")? as usize;
                    let attr_count =
                        read_u16_le(data, pos + hs + 12, "axml start element")? as usize;

                    let attr_size = if attr_size == 0 { 20 } else { attr_size };
                    let attrs_offset = pos + hs + 16;
                    let mut attributes = Vec::with_capacity(attr_count);
                    for j in 0..attr_count {
                        let ao = attrs_offset + j * attr_size;
                        require_len(data, ao, 20, "axml attribute")?;
                        let attr_ns = optional_idx(read_u32_le(data, ao, "axml attribute")?);
                        let attr_name = read_u32_le(data, ao + 4, "axml attribute")?;
                        let attr_raw = optional_idx(read_u32_le(data, ao + 8, "axml attribute")?);
                        let _tv_size = read_u16_le(data, ao + 12, "axml attribute")?;
                        let _tv_res0 = read_u8(data, ao + 14, "axml attribute")?;
                        let tv_type = read_u8(data, ao + 15, "axml attribute")?;
                        let tv_data = read_u32_le(data, ao + 16, "axml attribute")?;

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
                CHUNK_END_ELEMENT => {
                    require_len(data, pos + hs, 8, "axml end element")?;
                    let namespace = optional_idx(read_u32_le(data, pos + hs, "axml end element")?);
                    let name = read_u32_le(data, pos + hs + 4, "axml end element")?;
                    elements.push(AxmlEvent::EndElement { namespace, name });
                }
                _ => {}
            }

            pos += cs;
        }

        let string_pool =
            string_pool.ok_or_else(|| invalid("axml document", "no string pool found"))?;

        Ok(AxmlDocument {
            string_pool,
            resource_ids,
            elements,
        })
    }

    pub fn string(&self, index: u32) -> Option<&str> {
        self.string_pool.get(index)
    }

    pub fn resource_id_for(&self, string_idx: u32) -> Option<u32> {
        self.resource_ids.get(string_idx as usize).copied()
    }

    pub fn package_name(&self) -> Option<&str> {
        self.find_root_attr_by_name("package")
            .and_then(|attr| self.attr_as_string(attr))
    }

    pub fn version_code(&self) -> Option<u32> {
        self.find_root_attr_by_res_id(RES_VERSION_CODE)
            .and_then(|attr| self.attr_as_int(attr))
    }

    pub fn version_name(&self) -> Option<&str> {
        self.find_root_attr_by_res_id(RES_VERSION_NAME)
            .and_then(|attr| self.attr_as_string(attr))
    }

    pub fn split_name(&self) -> Option<&str> {
        self.find_root_attr_by_res_id(RES_SPLIT)
            .or_else(|| self.find_root_attr_by_name("split"))
            .and_then(|attr| self.attr_as_string(attr))
    }

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

    fn is_attr_res_id(&self, attr: &AxmlAttribute, res_id: u32) -> bool {
        self.resource_id_for(attr.name) == Some(res_id)
    }

    fn attr_as_string(&self, attr: &AxmlAttribute) -> Option<&str> {
        if let Some(raw) = attr.raw_value {
            if let Some(s) = self.string_pool.get(raw) {
                return Some(s);
            }
        }
        if let TypedValue::String(idx) = attr.typed_value {
            return self.string_pool.get(idx);
        }
        None
    }

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
