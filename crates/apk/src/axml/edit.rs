// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Element and attribute queries and mutations. Elements are addressed by
//! the index of their `StartElement` event.

use super::{AxmlAttribute, AxmlDocument, AxmlEvent};
use crate::value::ResValue;

impl AxmlDocument {
    pub fn root(&self) -> Option<usize> {
        self.elements
            .iter()
            .position(|event| matches!(event, AxmlEvent::StartElement { .. }))
    }

    pub fn element_name(&self, index: usize) -> Option<std::borrow::Cow<'_, str>> {
        match self.elements.get(index)? {
            AxmlEvent::StartElement { name, .. } => self.string(*name),
            _ => None,
        }
    }

    pub fn find_element(&self, name: &str) -> Option<usize> {
        (0..self.elements.len()).find(|&i| self.element_name(i).as_deref() == Some(name))
    }

    pub fn find_element_with_attr(&self, name: &str, res_id: u32, value: &str) -> Option<usize> {
        (0..self.elements.len()).find(|&i| {
            self.element_name(i).as_deref() == Some(name)
                && self
                    .attribute(i, res_id)
                    .is_some_and(|attr| self.attribute_string(attr).as_deref() == Some(value))
        })
    }

    pub fn find_end_element(&self, start: usize) -> Option<usize> {
        let AxmlEvent::StartElement {
            namespace, name, ..
        } = self.elements.get(start)?
        else {
            return None;
        };
        let mut depth = 0u32;
        for (i, event) in self.elements.iter().enumerate().skip(start) {
            match event {
                AxmlEvent::StartElement {
                    namespace: ns,
                    name: n,
                    ..
                } if ns == namespace && n == name => depth += 1,
                AxmlEvent::EndElement {
                    namespace: ns,
                    name: n,
                } if ns == namespace && n == name => {
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

    pub fn attributes(&self, index: usize) -> &[AxmlAttribute] {
        match self.elements.get(index) {
            Some(AxmlEvent::StartElement { attributes, .. }) => attributes,
            _ => &[],
        }
    }

    fn attributes_mut(&mut self, index: usize) -> Option<&mut Vec<AxmlAttribute>> {
        match self.elements.get_mut(index)? {
            AxmlEvent::StartElement { attributes, .. } => Some(attributes),
            _ => None,
        }
    }

    pub fn attribute(&self, index: usize, res_id: u32) -> Option<&AxmlAttribute> {
        self.attributes(index)
            .iter()
            .find(|attr| self.resource_id_for(attr.name) == Some(res_id))
    }

    pub fn attribute_named(&self, index: usize, name: &str) -> Option<&AxmlAttribute> {
        self.attributes(index)
            .iter()
            .find(|attr| self.string(attr.name).as_deref() == Some(name))
    }

    pub fn set_attribute(&mut self, index: usize, res_id: u32, value: ResValue) -> bool {
        let position = self
            .attributes(index)
            .iter()
            .position(|attr| self.resource_id_for(attr.name) == Some(res_id));
        match position.and_then(|p| self.attributes_mut(index).map(|attrs| &mut attrs[p])) {
            Some(attr) => {
                attr.set_value(value);
                true
            }
            None => false,
        }
    }

    pub fn add_attribute(&mut self, index: usize, attr: AxmlAttribute) -> bool {
        match self.attributes_mut(index) {
            Some(attrs) => {
                attrs.push(attr);
                true
            }
            None => false,
        }
    }

    /// An `android:` attribute named `name` with framework id `res_id`.
    pub fn make_attribute(&mut self, name: &str, res_id: u32, value: ResValue) -> AxmlAttribute {
        let name_index = self.intern_string(name);
        self.bind_resource_id(name_index, res_id);
        AxmlAttribute::new(self.android_ns(), name_index, value)
    }

    pub fn make_string_attribute(&mut self, name: &str, res_id: u32, value: &str) -> AxmlAttribute {
        let value = ResValue::string(self.intern_string(value));
        self.make_attribute(name, res_id, value)
    }

    pub(crate) fn insert_element(
        &mut self,
        position: usize,
        name: &str,
        attributes: Vec<AxmlAttribute>,
    ) {
        let name = self.intern_string(name);
        self.elements.splice(
            position..position,
            [
                AxmlEvent::StartElement {
                    namespace: None,
                    name,
                    attributes,
                },
                AxmlEvent::EndElement {
                    namespace: None,
                    name,
                },
            ],
        );
    }

    pub fn insert_child_element(
        &mut self,
        parent: usize,
        name: &str,
        attributes: Vec<AxmlAttribute>,
    ) {
        self.insert_element(parent + 1, name, attributes);
    }

    pub(crate) fn append_child_element(
        &mut self,
        parent: usize,
        name: &str,
        attributes: Vec<AxmlAttribute>,
    ) -> bool {
        match self.find_end_element(parent) {
            Some(end) => {
                self.insert_element(end, name, attributes);
                true
            }
            None => false,
        }
    }

    pub fn remove_element(&mut self, start: usize) -> bool {
        match self.find_end_element(start) {
            Some(end) => {
                self.elements.drain(start..=end);
                true
            }
            None => false,
        }
    }
}
