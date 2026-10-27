// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_storage::Bytes;
use std::borrow::Cow;
use std::ops::Range;

use super::ANDROID_NS;
use crate::res_value::ResValue;
use crate::string_pool::{StringEncoding, StringPool};

/// A binary XML document as its flat event stream. Names and string values
/// are indices into `string_pool`; `resource_ids[i]` is the framework
/// resource id of attribute name `i`, so those names come first in the pool.
#[derive(Debug, Clone)]
pub struct AxmlDocument {
    pub(super) string_pool: StringPool,
    pub(super) resource_ids: Vec<u32>,
    pub(super) elements: Vec<AxmlEvent>,
    pub(super) header: Vec<u8>,
    pub(super) resource_header: Vec<u8>,
    pub(super) suffix: NodeMetadata,
}

#[derive(Debug, Clone)]
pub enum AxmlEvent {
    StartNamespace {
        metadata: NodeMetadata,
        prefix: Option<u32>,
        uri: u32,
    },
    EndNamespace {
        metadata: NodeMetadata,
        prefix: Option<u32>,
        uri: u32,
    },
    StartElement {
        metadata: NodeMetadata,
        namespace: Option<u32>,
        name: u32,
        attributes: Vec<AxmlAttribute>,
    },
    EndElement {
        metadata: NodeMetadata,
        namespace: Option<u32>,
        name: u32,
    },
    Text {
        metadata: NodeMetadata,
        text: u32,
        value: ResValue,
    },
    /// An unmodeled chunk retained verbatim. It cannot be adopted into a
    /// different string pool because its string references are unknown.
    Opaque(NodeMetadata),
    #[doc(hidden)]
    StringPool,
    #[doc(hidden)]
    ResourceMap,
}

/// Original node bytes. Newly created nodes use zero line numbers and no
/// comment; parsed nodes retain header extensions and their original metadata.
#[derive(Debug, Clone, Default)]
pub struct NodeMetadata {
    pub(super) data: Bytes,
    pub(super) range: Range<usize>,
}

impl NodeMetadata {
    pub(super) fn bytes(&self) -> &[u8] {
        &self.data.as_bytes()[self.range.clone()]
    }
}

/// An attribute of a start element. A string value's raw text index follows
/// its typed value, so the value changes only through [`Self::set_value`].
#[derive(Debug, Clone)]
pub struct AxmlAttribute {
    pub(super) namespace: Option<u32>,
    pub(super) name: u32,
    pub(super) raw_value: Option<u32>,
    pub(super) value: ResValue,
    /// The attribute as parsed, including any bytes past the 20 Android reads.
    pub(super) encoded: Vec<u8>,
    pub(super) roles: AttributeRoles,
}

bitflags::bitflags! {
    /// The special attributes a start element names by index.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub(super) struct AttributeRoles: u8 {
        const ID = 1;
        const CLASS = 1 << 1;
        const STYLE = 1 << 2;
    }
}

impl AttributeRoles {
    /// Each role with the offset of its 1-based attribute index in a start
    /// element's extension; 0 there means no attribute has the role.
    pub(super) const FIELDS: [(Self, usize); 3] =
        [(Self::ID, 14), (Self::CLASS, 16), (Self::STYLE, 18)];
}

impl AxmlDocument {
    pub fn new(encoding: StringEncoding) -> Self {
        Self {
            string_pool: StringPool::new(Vec::new(), encoding),
            resource_ids: Vec::new(),
            elements: Vec::new(),
            header: Vec::new(),
            resource_header: Vec::new(),
            suffix: NodeMetadata::default(),
        }
    }

    pub fn string_pool(&self) -> &StringPool {
        &self.string_pool
    }
    pub fn string_pool_mut(&mut self) -> &mut StringPool {
        &mut self.string_pool
    }
    pub fn resource_ids(&self) -> &[u32] {
        &self.resource_ids
    }
    pub fn events(&self) -> &[AxmlEvent] {
        &self.elements
    }

    pub fn string(&self, index: u32) -> Option<Cow<'_, str>> {
        self.string_pool
            .get(index)
            .expect("document strings are validated at parse")
    }

    pub fn intern_string(&mut self, value: &str) -> u32 {
        self.string_pool
            .intern(value)
            .expect("document strings are validated at parse")
    }

    pub fn resource_id_for(&self, name: u32) -> Option<u32> {
        self.resource_ids
            .get(name as usize)
            .copied()
            .filter(|&id| id != 0)
    }

    /// The URI index of the declared Android framework namespace.
    pub fn android_ns(&self) -> Option<u32> {
        self.namespace_index(ANDROID_NS)
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
            encoded: Vec::new(),
            roles: AttributeRoles::empty(),
        }
    }

    pub fn namespace(&self) -> Option<u32> {
        self.namespace
    }

    pub fn name(&self) -> u32 {
        self.name
    }

    pub fn value(&self) -> ResValue {
        self.value
    }

    pub fn set_value(&mut self, value: ResValue) {
        self.raw_value = value.string_index();
        self.value = value;
    }
}
