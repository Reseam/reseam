// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;

use crate::string_pool::StringPool;
use crate::value::ResValue;

/// A binary XML document as its flat event stream. Names and string values
/// are indices into `string_pool`; `resource_ids[i]` is the framework
/// resource id of attribute name `i`, so those names come first in the pool.
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
    pub value: ResValue,
}

impl AxmlDocument {
    pub fn new(is_utf8: bool) -> Self {
        Self {
            string_pool: StringPool::new(Vec::new(), is_utf8),
            resource_ids: Vec::new(),
            elements: Vec::new(),
        }
    }

    pub fn string(&self, index: u32) -> Option<Cow<'_, str>> {
        self.string_pool.get(index)
    }

    pub fn intern_string(&mut self, value: &str) -> u32 {
        self.string_pool.intern(value)
    }

    pub(crate) fn resource_id_for(&self, name: u32) -> Option<u32> {
        self.resource_ids
            .get(name as usize)
            .copied()
            .filter(|&id| id != 0)
    }

    /// Records `res_id` as the framework id of attribute name `name`.
    pub(crate) fn bind_resource_id(&mut self, name: u32, res_id: u32) {
        let index = name as usize;
        if self.resource_ids.len() <= index {
            self.resource_ids.resize(index + 1, 0);
        }
        self.resource_ids[index] = res_id;
    }

    /// The uri index of the first declared namespace.
    pub fn android_ns(&self) -> Option<u32> {
        self.elements.iter().find_map(|event| match event {
            AxmlEvent::StartNamespace { uri, .. } => Some(*uri),
            _ => None,
        })
    }

    /// An attribute's string: its raw value when present, else its typed
    /// string.
    pub fn attribute_string(&self, attr: &AxmlAttribute) -> Option<Cow<'_, str>> {
        attr.raw_value
            .or_else(|| attr.value.string_index())
            .and_then(|index| self.string(index))
    }
}

impl AxmlAttribute {
    pub fn new(namespace: Option<u32>, name: u32, value: ResValue) -> Self {
        Self {
            namespace,
            name,
            raw_value: value.string_index(),
            value,
        }
    }

    pub fn set_value(&mut self, value: ResValue) {
        self.raw_value = value.string_index();
        self.value = value;
    }
}
