// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod adopt;

use super::NodeMetadata;

use super::{ANDROID_NS, AxmlAttribute, AxmlDocument, AxmlEvent};
use crate::error::{Result, invalid};
use crate::resources::ResourceTable;
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
                    ..
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

    /// Removes a bound attribute, retaining the roles of all remaining attributes.
    pub fn remove_attribute(&mut self, index: usize, res_id: u32) -> bool {
        self.edit_attributes(index, |document, attributes| {
            let count = attributes.len();
            attributes.retain(|attr| document.resource_id_for(attr.name) != Some(res_id));
            attributes.len() != count
        })
        .unwrap_or(false)
    }

    pub fn add_attribute(&mut self, index: usize, attr: AxmlAttribute) -> bool {
        self.edit_attributes(index, |document, attributes| {
            document.insert_attribute(attributes, attr);
        })
        .is_some()
    }

    /// Inserts `attr` where aapt would write it. Attributes with a resource id
    /// come first in ascending id order, and the framework's attribute lookup
    /// walks an element's attributes in that order, so one out of place is
    /// never found and silently takes its default.
    pub fn insert_attribute(&self, attributes: &mut Vec<AxmlAttribute>, attr: AxmlAttribute) {
        let position = self.attribute_position(attributes, &attr);
        attributes.insert(position, attr);
    }

    fn attribute_position(&self, attributes: &[AxmlAttribute], attr: &AxmlAttribute) -> usize {
        let rank = self.attribute_rank(attr);
        attributes
            .iter()
            .position(|existing| self.attribute_rank(existing) > rank)
            .unwrap_or(attributes.len())
    }

    fn sort_attributes(&self, attributes: &mut [AxmlAttribute]) {
        attributes.sort_by_key(|attr| self.attribute_rank(attr));
    }

    fn attribute_rank(&self, attr: &AxmlAttribute) -> u32 {
        self.resource_id_for(attr.name).unwrap_or(u32::MAX)
    }

    /// The uri index declared for `prefix`; for `android:` also the framework
    /// uri under whichever prefix the document declared it.
    pub fn declared_namespace(&self, prefix: &str) -> Option<u32> {
        self.elements.iter().find_map(|event| match event {
            AxmlEvent::StartNamespace {
                prefix: Some(name),
                uri,
                ..
            } if self.string(*name).as_deref() == Some(prefix) => Some(*uri),
            AxmlEvent::StartNamespace { uri, .. }
                if prefix == "android" && self.string(*uri).as_deref() == Some(ANDROID_NS) =>
            {
                Some(*uri)
            }
            _ => None,
        })
    }

    /// The namespace URI active at an event boundary. Inner declarations
    /// override outer declarations until their matching end event.
    pub fn declared_namespace_at(&self, index: usize, prefix: &str) -> Option<u32> {
        let mut scope = Vec::new();
        for event in self.elements.iter().take(index + 1) {
            match event {
                AxmlEvent::StartNamespace { prefix, uri, .. } => scope.push((*prefix, *uri)),
                AxmlEvent::EndNamespace { prefix, uri, .. } => {
                    if let Some(i) = scope.iter().rposition(|entry| *entry == (*prefix, *uri)) {
                        scope.remove(i);
                    }
                }
                _ => {}
            }
        }
        scope
            .iter()
            .rev()
            .find_map(|&(name, uri)| {
                (name.and_then(|name| self.string(name)).as_deref() == Some(prefix)).then_some(uri)
            })
            .or_else(|| {
                (prefix == "android")
                    .then(|| {
                        scope.iter().rev().find_map(|&(_, uri)| {
                            (self.string(uri).as_deref() == Some(ANDROID_NS)).then_some(uri)
                        })
                    })
                    .flatten()
            })
    }

    /// The index of the uri the document declares a namespace for.
    pub fn namespace_index(&self, uri: &str) -> Option<u32> {
        self.elements.iter().find_map(|event| match event {
            AxmlEvent::StartNamespace { uri: index, .. }
                if self.string(*index).as_deref() == Some(uri) =>
            {
                Some(*index)
            }
            _ => None,
        })
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

    /// Returns namespace and name indices, binding prefixed names to resource ids.
    /// `android:` uses the framework table; other prefixes use `resources`.
    /// Fails for unresolved prefixed names. Unqualified names have no resource id.
    pub fn bind_attribute_name(
        &mut self,
        name: &str,
        resources: Option<&ResourceTable>,
    ) -> Result<(Option<u32>, u32)> {
        self.bind_attribute_name_at(self.root().unwrap_or(0), name, resources)
    }

    /// Binds an attribute against namespaces active at the target element.
    pub fn bind_attribute_name_at(
        &mut self,
        index: usize,
        name: &str,
        resources: Option<&ResourceTable>,
    ) -> Result<(Option<u32>, u32)> {
        let Some((prefix, local)) = name.split_once(':') else {
            return Ok((None, self.intern_string(name)));
        };
        let namespace = self
            .declared_namespace_at(index, prefix)
            .or_else(|| (prefix == "android").then(|| self.intern_string(ANDROID_NS)))
            .ok_or_else(|| {
                invalid(
                    "axml attribute",
                    format!(
                        "attribute {name}: the document declares no xmlns:{prefix}; declare it with declareNamespace(\"{prefix}\", uri)"
                    ),
                )
            })?;
        self.bind_local_name(name, namespace, local, resources)
    }

    /// [`Self::bind_attribute_name`] with a namespace URI; `None` is unqualified.
    pub fn bind_attribute(
        &mut self,
        uri: Option<&str>,
        local: &str,
        resources: Option<&ResourceTable>,
    ) -> Result<(Option<u32>, u32)> {
        let Some(uri) = uri else {
            return Ok((None, self.intern_string(local)));
        };
        let namespace = self.namespace_index(uri).ok_or_else(|| {
            invalid(
                "axml attribute",
                format!(
                    "attribute {local}: the document declares no namespace {uri}; declare it with declareNamespace(prefix, \"{uri}\")"
                ),
            )
        })?;
        self.bind_local_name(local, namespace, local, resources)
    }

    fn bind_local_name(
        &mut self,
        name: &str,
        namespace: u32,
        local: &str,
        resources: Option<&ResourceTable>,
    ) -> Result<(Option<u32>, u32)> {
        let uri = self
            .string(namespace)
            .ok_or_else(|| invalid("XML namespace", "invalid URI index"))?;
        let res_id = super::compiler::attribute_resource_id(Some(&uri), local, resources)?
            .ok_or_else(|| {
                invalid(
                    "XML attribute",
                    format!("attribute {name} has no resource ID"),
                )
            })?;
        Ok((Some(namespace), self.intern_attribute_name(local, res_id)))
    }

    pub(super) fn intern_attribute_name(&mut self, name: &str, res_id: u32) -> u32 {
        let existing = self
            .resource_ids
            .iter()
            .enumerate()
            .find_map(|(index, &id)| {
                (id == res_id && self.string(index as u32).as_deref() == Some(name))
                    .then_some(index as u32)
            });
        let index = existing.unwrap_or_else(|| self.string_pool.push(name));
        self.bind_resource_id(index, res_id);
        index
    }

    /// An `android:` attribute named `name` with framework id `res_id`.
    pub fn make_attribute(&mut self, name: &str, res_id: u32, value: ResValue) -> AxmlAttribute {
        let name_index = self.intern_attribute_name(name, res_id);
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
        mut attributes: Vec<AxmlAttribute>,
    ) {
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

    /// Inserts an element as the first child of `parent`.
    /// An absent or unterminated parent is an error and leaves the document unchanged.
    pub fn insert_child_element(
        &mut self,
        parent: usize,
        name: &str,
        attributes: Vec<AxmlAttribute>,
    ) -> Result<()> {
        self.find_end_element(parent)
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
