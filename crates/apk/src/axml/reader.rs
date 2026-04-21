// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::axml::string_pool::StringPool;
use crate::axml::{
    CHUNK_END_ELEMENT, CHUNK_END_NAMESPACE, CHUNK_RESOURCE_IDS, CHUNK_START_ELEMENT,
    CHUNK_START_NAMESPACE, CHUNK_STRING_POOL, CHUNK_XML_DOCUMENT, TYPE_INT_BOOLEAN, TYPE_INT_DEC,
    TYPE_INT_HEX, TYPE_REFERENCE, TYPE_STRING,
};
use crate::buf::{read_u16_le, read_u32_le, read_u8, require_len};
use crate::error::{invalid, malformed, Result};

const RES_VERSION_CODE: u32 = 0x0101_021b;
const RES_VERSION_NAME: u32 = 0x0101_021c;
const RES_MIN_SDK_VERSION: u32 = 0x0101_020c;
const RES_SPLIT: u32 = 0x0101_048a;

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

            if cs < 8 || hs < 8 || hs > cs || pos + cs > data.len() {
                return Err(malformed(
                    "axml chunk",
                    pos,
                    "chunk extends past end of document",
                ));
            }
            let chunk = &data[pos..pos + cs];

            match ct {
                CHUNK_STRING_POOL => {
                    let chunk_body = &chunk[8..];
                    string_pool = Some(StringPool::parse(chunk_body, pos)?);
                }
                CHUNK_RESOURCE_IDS => {
                    let count = (cs.saturating_sub(hs)) / 4;
                    let mut ids = Vec::with_capacity(count);
                    for i in 0..count {
                        ids.push(read_u32_le(chunk, hs + i * 4, "axml resource ids")?);
                    }
                    resource_ids = ids;
                }
                CHUNK_START_NAMESPACE => {
                    require_len(chunk, hs, 8, "axml namespace")?;
                    let prefix = optional_idx(read_u32_le(chunk, hs, "axml namespace")?);
                    let uri = read_u32_le(chunk, hs + 4, "axml namespace")?;
                    elements.push(AxmlEvent::StartNamespace { prefix, uri });
                }
                CHUNK_END_NAMESPACE => {
                    require_len(chunk, hs, 8, "axml namespace")?;
                    let prefix = optional_idx(read_u32_le(chunk, hs, "axml namespace")?);
                    let uri = read_u32_le(chunk, hs + 4, "axml namespace")?;
                    elements.push(AxmlEvent::EndNamespace { prefix, uri });
                }
                CHUNK_START_ELEMENT => {
                    require_len(chunk, hs, 20, "axml start element")?;
                    let namespace = optional_idx(read_u32_le(chunk, hs, "axml start element")?);
                    let name = read_u32_le(chunk, hs + 4, "axml start element")?;
                    let attr_start = read_u16_le(chunk, hs + 8, "axml start element")? as usize;
                    let attr_size = read_u16_le(chunk, hs + 10, "axml start element")? as usize;
                    let attr_count = read_u16_le(chunk, hs + 12, "axml start element")? as usize;

                    let attr_size = if attr_size == 0 { 20 } else { attr_size };
                    if attr_size < 20 {
                        return Err(malformed(
                            "axml start element",
                            pos + hs + 10,
                            "attribute size is smaller than 20 bytes",
                        ));
                    }
                    let attrs_offset = hs + attr_start;
                    let attrs_len = attr_count.checked_mul(attr_size).ok_or_else(|| {
                        malformed(
                            "axml start element",
                            pos + hs + 12,
                            "attribute data overflows chunk size",
                        )
                    })?;
                    require_len(chunk, attrs_offset, attrs_len, "axml attributes")?;
                    let mut attributes = Vec::with_capacity(attr_count);
                    for j in 0..attr_count {
                        let ao = attrs_offset + j * attr_size;
                        require_len(chunk, ao, 20, "axml attribute")?;
                        let attr_ns = optional_idx(read_u32_le(chunk, ao, "axml attribute")?);
                        let attr_name = read_u32_le(chunk, ao + 4, "axml attribute")?;
                        let attr_raw = optional_idx(read_u32_le(chunk, ao + 8, "axml attribute")?);
                        let _tv_size = read_u16_le(chunk, ao + 12, "axml attribute")?;
                        let _tv_res0 = read_u8(chunk, ao + 14, "axml attribute")?;
                        let tv_type = read_u8(chunk, ao + 15, "axml attribute")?;
                        let tv_data = read_u32_le(chunk, ao + 16, "axml attribute")?;

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
                    require_len(chunk, hs, 8, "axml end element")?;
                    let namespace = optional_idx(read_u32_le(chunk, hs, "axml end element")?);
                    let name = read_u32_le(chunk, hs + 4, "axml end element")?;
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
            TypedValue::Int(v) if v >= 0 => Some(v as u32),
            TypedValue::Hex(v) => Some(v),
            _ => None,
        }
    }

    pub fn intern_string(&mut self, s: &str) -> u32 {
        self.string_pool.intern(s)
    }

    pub fn set_version_code(&mut self, code: u32) {
        self.set_root_attr_int(RES_VERSION_CODE, code as i32);
    }

    pub fn set_version_name(&mut self, name: &str) {
        let idx = self.string_pool.intern(name);
        self.set_root_attr_string(RES_VERSION_NAME, idx);
    }

    pub fn set_min_sdk(&mut self, sdk: u32) {
        for event in &mut self.elements {
            if let AxmlEvent::StartElement {
                name, attributes, ..
            } = event
            {
                if self.string_pool.get(*name) == Some("uses-sdk") {
                    for attr in attributes.iter_mut() {
                        if self.resource_ids.get(attr.name as usize) == Some(&RES_MIN_SDK_VERSION) {
                            attr.typed_value = TypedValue::Int(sdk as i32);
                            return;
                        }
                    }
                }
            }
        }
    }

    pub fn add_permission(&mut self, permission: &str) {
        let name_idx = self.string_pool.intern("uses-permission");
        let attr_name_idx = self.string_pool.intern("name");
        let android_ns_idx = self.elements.iter().find_map(|e| {
            if let AxmlEvent::StartNamespace { uri, .. } = e {
                Some(*uri)
            } else {
                None
            }
        });

        let perm_str_idx = self.string_pool.intern(permission);

        let attr = AxmlAttribute {
            namespace: android_ns_idx,
            name: attr_name_idx,
            raw_value: Some(perm_str_idx),
            typed_value: TypedValue::String(perm_str_idx),
        };

        // Insert before the closing </manifest>
        let insert_pos = self
            .elements
            .iter()
            .rposition(|e| matches!(e, AxmlEvent::EndElement { .. }))
            .unwrap_or(self.elements.len());

        self.elements.insert(
            insert_pos,
            AxmlEvent::EndElement {
                namespace: None,
                name: name_idx,
            },
        );
        self.elements.insert(
            insert_pos,
            AxmlEvent::StartElement {
                namespace: None,
                name: name_idx,
                attributes: vec![attr],
            },
        );
    }

    pub fn set_attribute_int(&mut self, element_name: &str, res_id: u32, value: i32) {
        for event in &mut self.elements {
            if let AxmlEvent::StartElement {
                name, attributes, ..
            } = event
            {
                if self.string_pool.get(*name) == Some(element_name) {
                    for attr in attributes.iter_mut() {
                        if self.resource_ids.get(attr.name as usize) == Some(&res_id) {
                            attr.typed_value = TypedValue::Int(value);
                            return;
                        }
                    }
                }
            }
        }
    }

    pub fn set_attribute_string(&mut self, element_name: &str, res_id: u32, value: &str) {
        let str_idx = self.string_pool.intern(value);
        for event in &mut self.elements {
            if let AxmlEvent::StartElement {
                name, attributes, ..
            } = event
            {
                if self.string_pool.get(*name) == Some(element_name) {
                    for attr in attributes.iter_mut() {
                        if self.resource_ids.get(attr.name as usize) == Some(&res_id) {
                            attr.raw_value = Some(str_idx);
                            attr.typed_value = TypedValue::String(str_idx);
                            return;
                        }
                    }
                }
            }
        }
    }

    // ── High-level manifest mutations ──

    pub fn android_ns(&self) -> Option<u32> {
        self.elements.iter().find_map(|e| {
            if let AxmlEvent::StartNamespace { uri, .. } = e {
                Some(*uri)
            } else {
                None
            }
        })
    }

    pub fn find_element_index(&self, element_name: &str) -> Option<usize> {
        self.elements.iter().position(|e| {
            if let AxmlEvent::StartElement { name, .. } = e {
                self.string_pool.get(*name) == Some(element_name)
            } else {
                false
            }
        })
    }

    pub fn find_element_with_attr(
        &self,
        element_name: &str,
        attr_res_id: u32,
        attr_value: &str,
    ) -> Option<usize> {
        self.elements.iter().position(|e| {
            if let AxmlEvent::StartElement {
                name, attributes, ..
            } = e
            {
                self.string_pool.get(*name) == Some(element_name)
                    && attributes.iter().any(|a| {
                        self.resource_ids.get(a.name as usize) == Some(&attr_res_id)
                            && self.attr_as_string(a) == Some(attr_value)
                    })
            } else {
                false
            }
        })
    }

    pub fn get_attribute_int(&self, element_idx: usize, res_id: u32) -> Option<u32> {
        if let Some(AxmlEvent::StartElement { attributes, .. }) = self.elements.get(element_idx) {
            for attr in attributes {
                if self.resource_ids.get(attr.name as usize) == Some(&res_id) {
                    return self.attr_as_int(attr);
                }
            }
        }
        None
    }

    pub fn get_attribute_string(&self, element_idx: usize, res_id: u32) -> Option<&str> {
        if let Some(AxmlEvent::StartElement { attributes, .. }) = self.elements.get(element_idx) {
            for attr in attributes {
                if self.resource_ids.get(attr.name as usize) == Some(&res_id) {
                    return self.attr_as_string(attr);
                }
            }
        }
        None
    }

    pub fn set_element_attribute_int(&mut self, element_idx: usize, res_id: u32, value: i32) {
        if let Some(AxmlEvent::StartElement { attributes, .. }) = self.elements.get_mut(element_idx)
        {
            for attr in attributes.iter_mut() {
                if self.resource_ids.get(attr.name as usize) == Some(&res_id) {
                    attr.typed_value = TypedValue::Int(value);
                    return;
                }
            }
        }
    }

    pub fn set_element_attribute_bool(&mut self, element_idx: usize, res_id: u32, value: bool) {
        if let Some(AxmlEvent::StartElement { attributes, .. }) = self.elements.get_mut(element_idx)
        {
            for attr in attributes.iter_mut() {
                if self.resource_ids.get(attr.name as usize) == Some(&res_id) {
                    attr.typed_value = TypedValue::Bool(value);
                    return;
                }
            }
        }
    }

    pub fn add_element_attribute_int(
        &mut self,
        element_idx: usize,
        attr_name: &str,
        res_id: u32,
        value: i32,
    ) {
        let name_idx = self.string_pool.intern(attr_name);
        let ns = self.android_ns();
        if let Some(AxmlEvent::StartElement { attributes, .. }) = self.elements.get_mut(element_idx)
        {
            attributes.push(AxmlAttribute {
                namespace: ns,
                name: name_idx,
                raw_value: None,
                typed_value: TypedValue::Int(value),
            });
        }
        // Ensure resource_ids maps this name_idx to the res_id
        let idx = name_idx as usize;
        if self.resource_ids.len() <= idx {
            self.resource_ids.resize(idx + 1, 0);
        }
        self.resource_ids[idx] = res_id;
    }

    pub fn add_element_attribute_string(
        &mut self,
        element_idx: usize,
        attr_name: &str,
        res_id: u32,
        value: &str,
    ) {
        let name_idx = self.string_pool.intern(attr_name);
        let value_idx = self.string_pool.intern(value);
        let ns = self.android_ns();
        if let Some(AxmlEvent::StartElement { attributes, .. }) = self.elements.get_mut(element_idx)
        {
            attributes.push(AxmlAttribute {
                namespace: ns,
                name: name_idx,
                raw_value: Some(value_idx),
                typed_value: TypedValue::String(value_idx),
            });
        }
        let idx = name_idx as usize;
        if self.resource_ids.len() <= idx {
            self.resource_ids.resize(idx + 1, 0);
        }
        self.resource_ids[idx] = res_id;
    }

    pub fn add_element_attribute_bool(
        &mut self,
        element_idx: usize,
        attr_name: &str,
        res_id: u32,
        value: bool,
    ) {
        let name_idx = self.string_pool.intern(attr_name);
        let ns = self.android_ns();
        if let Some(AxmlEvent::StartElement { attributes, .. }) = self.elements.get_mut(element_idx)
        {
            attributes.push(AxmlAttribute {
                namespace: ns,
                name: name_idx,
                raw_value: None,
                typed_value: TypedValue::Bool(value),
            });
        }
        let idx = name_idx as usize;
        if self.resource_ids.len() <= idx {
            self.resource_ids.resize(idx + 1, 0);
        }
        self.resource_ids[idx] = res_id;
    }

    pub fn find_end_element(&self, start_idx: usize) -> Option<usize> {
        if start_idx >= self.elements.len() {
            return None;
        }
        let (target_ns, target_name) = match &self.elements[start_idx] {
            AxmlEvent::StartElement {
                namespace, name, ..
            } => (*namespace, *name),
            _ => return None,
        };
        let mut depth = 0u32;
        for i in start_idx..self.elements.len() {
            match &self.elements[i] {
                AxmlEvent::StartElement {
                    namespace, name, ..
                } if *namespace == target_ns && *name == target_name => {
                    depth += 1;
                }
                AxmlEvent::EndElement { namespace, name }
                    if *namespace == target_ns && *name == target_name =>
                {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub fn insert_element_before(
        &mut self,
        position: usize,
        element_name: &str,
        attributes: Vec<AxmlAttribute>,
    ) {
        let name_idx = self.string_pool.intern(element_name);
        self.elements.insert(
            position,
            AxmlEvent::EndElement {
                namespace: None,
                name: name_idx,
            },
        );
        self.elements.insert(
            position,
            AxmlEvent::StartElement {
                namespace: None,
                name: name_idx,
                attributes,
            },
        );
    }

    pub fn insert_child_element(
        &mut self,
        parent_start_idx: usize,
        element_name: &str,
        attributes: Vec<AxmlAttribute>,
    ) {
        let insert_pos = parent_start_idx + 1;
        let name_idx = self.string_pool.intern(element_name);
        self.elements.insert(
            insert_pos,
            AxmlEvent::StartElement {
                namespace: None,
                name: name_idx,
                attributes,
            },
        );
        self.elements.insert(
            insert_pos + 1,
            AxmlEvent::EndElement {
                namespace: None,
                name: name_idx,
            },
        );
    }

    pub fn remove_element(&mut self, start_idx: usize) -> bool {
        if let Some(end_idx) = self.find_end_element(start_idx) {
            self.elements.drain(start_idx..=end_idx);
            true
        } else {
            false
        }
    }

    pub fn make_attribute(
        &mut self,
        attr_name: &str,
        res_id: u32,
        value: TypedValue,
    ) -> AxmlAttribute {
        let name_idx = self.string_pool.intern(attr_name);
        let ns = self.android_ns();
        let raw_value = match &value {
            TypedValue::String(idx) => Some(*idx),
            _ => None,
        };
        let idx = name_idx as usize;
        if self.resource_ids.len() <= idx {
            self.resource_ids.resize(idx + 1, 0);
        }
        self.resource_ids[idx] = res_id;
        AxmlAttribute {
            namespace: ns,
            name: name_idx,
            raw_value,
            typed_value: value,
        }
    }

    pub fn make_string_attribute(
        &mut self,
        attr_name: &str,
        res_id: u32,
        value: &str,
    ) -> AxmlAttribute {
        let value_idx = self.string_pool.intern(value);
        self.make_attribute(attr_name, res_id, TypedValue::String(value_idx))
    }

    pub fn make_int_attribute(
        &mut self,
        attr_name: &str,
        res_id: u32,
        value: i32,
    ) -> AxmlAttribute {
        self.make_attribute(attr_name, res_id, TypedValue::Int(value))
    }

    pub fn make_bool_attribute(
        &mut self,
        attr_name: &str,
        res_id: u32,
        value: bool,
    ) -> AxmlAttribute {
        self.make_attribute(attr_name, res_id, TypedValue::Bool(value))
    }

    // ── Private helpers ──

    fn set_root_attr_int(&mut self, res_id: u32, value: i32) {
        let first_element = self
            .elements
            .iter_mut()
            .find(|e| matches!(e, AxmlEvent::StartElement { .. }));
        if let Some(AxmlEvent::StartElement { attributes, .. }) = first_element {
            for attr in attributes.iter_mut() {
                if self.resource_ids.get(attr.name as usize) == Some(&res_id) {
                    attr.typed_value = TypedValue::Int(value);
                    return;
                }
            }
        }
    }

    fn set_root_attr_string(&mut self, res_id: u32, str_idx: u32) {
        let first_element = self
            .elements
            .iter_mut()
            .find(|e| matches!(e, AxmlEvent::StartElement { .. }));
        if let Some(AxmlEvent::StartElement { attributes, .. }) = first_element {
            for attr in attributes.iter_mut() {
                if self.resource_ids.get(attr.name as usize) == Some(&res_id) {
                    attr.raw_value = Some(str_idx);
                    attr.typed_value = TypedValue::String(str_idx);
                    return;
                }
            }
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
