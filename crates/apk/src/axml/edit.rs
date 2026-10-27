// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::tree::element_end;
use super::{AxmlAttribute, AxmlDocument, AxmlEvent, NodeMetadata};
use crate::error::{Result, invalid};
use crate::res_value::ResValue;

impl AxmlDocument {
    /// Sets the value of the attribute with framework id `res_id`; false when
    /// the element has none.
    pub fn set_attribute(&mut self, index: usize, res_id: u32, value: ResValue) -> bool {
        let Some(position) = self
            .attributes(index)
            .iter()
            .position(|attr| self.resource_id_for(attr.name) == Some(res_id))
        else {
            return false;
        };
        match self.elements.get_mut(index) {
            Some(AxmlEvent::StartElement { attributes, .. }) => {
                attributes[position].set_value(value);
                true
            }
            _ => false,
        }
    }

    /// Sets the `android:` attribute with framework id `res_id`, or adds it as
    /// `name` where aapt would write it; false when `index` is not an element.
    pub fn put_android_attribute(
        &mut self,
        index: usize,
        name: &str,
        res_id: u32,
        value: ResValue,
    ) -> bool {
        if self.set_attribute(index, res_id, value) {
            return true;
        }
        let attr = self.make_attribute(name, res_id, value);
        self.add_attribute(index, attr)
    }

    pub fn add_attribute(&mut self, index: usize, attr: AxmlAttribute) -> bool {
        self.edit_attributes(index, |document, attributes| {
            document.insert_attribute(attributes, attr);
        })
        .is_some()
    }

    /// Sets the attribute that is `name` in `namespace`, as [`Self::is_attribute`]
    /// matches it, or inserts a new one where aapt would write it.
    pub fn put_attribute(
        &self,
        attributes: &mut Vec<AxmlAttribute>,
        namespace: Option<u32>,
        name: u32,
        value: ResValue,
    ) {
        let existing = self.string(name).and_then(|local| {
            attributes
                .iter()
                .position(|attr| self.is_attribute(attr, namespace, &local))
        });
        match existing {
            Some(index) => attributes[index].set_value(value),
            None => self.insert_attribute(attributes, AxmlAttribute::new(namespace, name, value)),
        }
    }

    /// Inserts `attr` where aapt would write it.
    fn insert_attribute(&self, attributes: &mut Vec<AxmlAttribute>, attr: AxmlAttribute) {
        let rank = self.attribute_rank(&attr);
        let position = attributes
            .iter()
            .position(|existing| self.attribute_rank(existing) > rank)
            .unwrap_or(attributes.len());
        attributes.insert(position, attr);
    }

    pub(super) fn sort_attributes(&self, attributes: &mut [AxmlAttribute]) {
        attributes.sort_by_key(|attr| self.attribute_rank(attr));
    }

    /// aapt writes attributes with a resource id first, in ascending id order.
    /// The framework's attribute lookup walks an element's attributes in that
    /// order, so one out of place is never found and silently takes its default.
    fn attribute_rank(&self, attr: &AxmlAttribute) -> u32 {
        self.resource_id_for(attr.name).unwrap_or(u32::MAX)
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

    /// Declares `prefix` for `uri`, unless the document already declares that
    /// prefix for it. Namespaces wrap the whole event stream, so the
    /// declaration goes in front of the root and closes after it, moving the
    /// index of every element resolved before the call.
    pub fn declare_namespace(&mut self, prefix: &str, uri: &str) -> Result<()> {
        if let Some(declared) = self.declared_namespace(prefix) {
            let declared = self.string(declared).unwrap_or_default();
            return if declared == uri {
                Ok(())
            } else {
                Err(invalid(
                    "axml namespace",
                    format!("the document already declares xmlns:{prefix} as {declared}"),
                ))
            };
        }
        let prefix = Some(self.intern_string(prefix));
        let uri = self.intern_string(uri);
        self.elements.insert(
            0,
            AxmlEvent::StartNamespace {
                metadata: NodeMetadata::default(),
                prefix,
                uri,
            },
        );
        self.elements.push(AxmlEvent::EndNamespace {
            metadata: NodeMetadata::default(),
            prefix,
            uri,
        });
        Ok(())
    }

    /// Inserts event nodes at a boundary. Document pools cannot be inserted
    /// through this operation. Out-of-range boundaries are errors.
    pub fn insert_events(
        &mut self,
        position: usize,
        events: impl IntoIterator<Item = AxmlEvent>,
    ) -> Result<()> {
        if position > self.elements.len() {
            return Err(invalid(
                "axml edit",
                "insertion boundary is outside the document",
            ));
        }
        let events = events.into_iter().collect::<Vec<_>>();
        if events
            .iter()
            .any(|event| matches!(event, AxmlEvent::StringPool | AxmlEvent::ResourceMap))
        {
            return Err(invalid(
                "axml edit",
                "cannot insert a document pool as a node",
            ));
        }
        self.elements.splice(position..position, events);
        Ok(())
    }

    /// Removes a complete element subtree, transferring its original metadata
    /// with it. An absent or unterminated element is an error.
    pub fn detach_element(&mut self, start: usize) -> Result<Vec<AxmlEvent>> {
        let end = element_end(&self.elements, start)
            .ok_or_else(|| invalid("axml edit", "element is absent or unterminated"))?;
        Ok(self.elements.drain(start..=end).collect())
    }

    /// Inserts an element as the first child of `parent`.
    /// An absent or unterminated parent is an error and leaves the document unchanged.
    pub fn insert_child_element(
        &mut self,
        parent: usize,
        name: &str,
        attributes: Vec<AxmlAttribute>,
    ) -> Result<()> {
        element_end(&self.elements, parent)
            .ok_or_else(|| invalid("axml edit", "parent is absent or unterminated"))?;
        self.insert_element(parent + 1, name, attributes);
        Ok(())
    }

    /// Inserts `name` as the last child of `parent`; false when `parent` is unterminated.
    pub fn append_child_element(
        &mut self,
        parent: usize,
        name: &str,
        attributes: Vec<AxmlAttribute>,
    ) -> bool {
        match element_end(&self.elements, parent) {
            Some(end) => {
                self.insert_element(end, name, attributes);
                true
            }
            None => false,
        }
    }

    fn insert_element(&mut self, position: usize, name: &str, mut attributes: Vec<AxmlAttribute>) {
        self.sort_attributes(&mut attributes);
        let name = self.intern_string(name);
        self.elements.splice(
            position..position,
            [
                AxmlEvent::StartElement {
                    metadata: NodeMetadata::default(),
                    namespace: None,
                    name,
                    attributes,
                },
                AxmlEvent::EndElement {
                    metadata: NodeMetadata::default(),
                    namespace: None,
                    name,
                },
            ],
        );
    }
}
