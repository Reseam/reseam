// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_storage::Bytes;
use std::borrow::Cow;
use std::ops::Range;

use super::ANDROID_NS;
use crate::string_pool::{StringEncoding, StringPool};
use crate::value::ResValue;

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

#[derive(Debug, Clone)]
pub struct AxmlAttribute {
    pub namespace: Option<u32>,
    pub name: u32,
    pub raw_value: Option<u32>,
    pub value: ResValue,
    pub(super) encoded: Vec<u8>,
    pub(super) roles: [bool; 3],
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

    /// Constructs a document over a validated string pool and event stream.
    pub fn from_parts(
        string_pool: StringPool,
        resource_ids: Vec<u32>,
        elements: Vec<AxmlEvent>,
    ) -> crate::Result<Self> {
        for string in string_pool.iter() {
            string?;
        }
        Ok(Self {
            string_pool,
            resource_ids,
            elements,
            header: Vec::new(),
            resource_header: Vec::new(),
            suffix: NodeMetadata::default(),
        })
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

    /// Inserts event nodes at a boundary. Document pools cannot be inserted
    /// through this operation. Out-of-range boundaries are errors.
    pub fn insert_events(
        &mut self,
        position: usize,
        events: impl IntoIterator<Item = AxmlEvent>,
    ) -> crate::Result<()> {
        if position > self.elements.len() {
            return Err(crate::error::invalid(
                "axml edit",
                "insertion boundary is outside the document",
            ));
        }
        let events = events.into_iter().collect::<Vec<_>>();
        if events
            .iter()
            .any(|event| matches!(event, AxmlEvent::StringPool | AxmlEvent::ResourceMap))
        {
            return Err(crate::error::invalid(
                "axml edit",
                "cannot insert a document pool as a node",
            ));
        }
        self.elements.splice(position..position, events);
        Ok(())
    }

    /// Removes a complete element subtree, transferring its original metadata
    /// with it. An absent or unterminated element is an error.
    pub fn detach_element(&mut self, start: usize) -> crate::Result<Vec<AxmlEvent>> {
        let end = self.find_end_element(start).ok_or_else(|| {
            crate::error::invalid("axml edit", "element is absent or unterminated")
        })?;
        Ok(self.elements.drain(start..=end).collect())
    }

    /// Edits an element's attributes while retaining its node metadata. The
    /// callback may intern strings and bind names but must not move event nodes.
    /// Returns `None` if the index is not an element or the callback removes it.
    pub fn edit_attributes<R>(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut Self, &mut Vec<AxmlAttribute>) -> R,
    ) -> Option<R> {
        let AxmlEvent::StartElement { attributes, .. } = self.elements.get_mut(index)? else {
            return None;
        };
        let mut attributes = std::mem::take(attributes);
        let result = edit(self, &mut attributes);
        let Some(AxmlEvent::StartElement {
            attributes: slot, ..
        }) = self.elements.get_mut(index)
        else {
            return None;
        };
        *slot = attributes;
        Some(result)
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

    pub(crate) fn bind_resource_id(&mut self, name: u32, res_id: u32) {
        let index = name as usize;
        if self.resource_ids.len() <= index {
            self.resource_ids.resize(index + 1, 0);
        }
        self.resource_ids[index] = res_id;
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
            roles: [false; 3],
        }
    }

    pub fn set_value(&mut self, value: ResValue) {
        self.raw_value = value.string_index();
        self.value = value;
    }
}
