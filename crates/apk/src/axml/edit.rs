// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Element and attribute queries and mutations. Elements are addressed by
//! the index of their `StartElement` event.

use super::{android_attr_res_id, AxmlAttribute, AxmlDocument, AxmlEvent, ANDROID_NS};
use crate::error::{invalid, Result};
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
        let Some(mut attributes) = self.attributes_mut(index).map(std::mem::take) else {
            return false;
        };
        self.insert_attribute(&mut attributes, attr);
        *self.attributes_mut(index).expect("checked above") = attributes;
        true
    }

    /// Inserts `attr` where aapt would write it. Attributes with a resource id
    /// come first in ascending id order, and the framework's attribute lookup
    /// walks an element's attributes in that order, so one out of place is
    /// never found and silently takes its default.
    pub fn insert_attribute(&self, attributes: &mut Vec<AxmlAttribute>, attr: AxmlAttribute) {
        let rank = self.attribute_rank(&attr);
        let position = attributes
            .iter()
            .position(|existing| self.attribute_rank(existing) > rank)
            .unwrap_or(attributes.len());
        attributes.insert(position, attr);
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
            } if self.string(*name).as_deref() == Some(prefix) => Some(*uri),
            AxmlEvent::StartNamespace { uri, .. }
                if prefix == "android" && self.string(*uri).as_deref() == Some(ANDROID_NS) =>
            {
                Some(*uri)
            }
            _ => None,
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
            return match declared == uri {
                true => Ok(()),
                false => Err(invalid(
                    "axml namespace",
                    format!("the document already declares xmlns:{prefix} as {declared}"),
                )),
            };
        }
        let prefix = Some(self.intern_string(prefix));
        let uri = self.intern_string(uri);
        self.elements
            .insert(0, AxmlEvent::StartNamespace { prefix, uri });
        self.elements.push(AxmlEvent::EndNamespace { prefix, uri });
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
        let Some((prefix, local)) = name.split_once(':') else {
            return Ok((None, self.intern_string(name)));
        };
        let namespace = self
            .declared_namespace(prefix)
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
        let framework = self.string(namespace).as_deref() == Some(ANDROID_NS);
        let res_id = if framework {
            android_attr_res_id(local)
        } else {
            resources.and_then(|table| table.find_resource_id("attr", local))
        }
        .ok_or_else(|| {
            let source = if framework {
                "the framework attribute table"
            } else {
                "the app's resource table"
            };
            invalid(
                "axml attribute",
                format!(
                    "attribute {name}: {source} has no id for it, and an attribute without one is ignored by the inflater"
                ),
            )
        })?;
        let name_index = self
            .string_pool
            .iter()
            .enumerate()
            .find_map(|(index, value)| {
                (value == local && self.resource_id_for(index as u32) == Some(res_id))
                    .then_some(index as u32)
            });
        let name_index = name_index.unwrap_or_else(|| self.string_pool.push(local));
        self.bind_resource_id(name_index, res_id);
        Ok((Some(namespace), name_index))
    }

    /// A deep copy of `events`, a subtree of `source`, in this document's own
    /// strings and namespaces. Every attribute is re-bound to the id this
    /// document resolves it by, so an adopted attribute carries what the
    /// inflater reads or the adoption fails.
    pub fn adopt(
        &mut self,
        source: &AxmlDocument,
        events: &[AxmlEvent],
        resources: Option<&ResourceTable>,
    ) -> Result<Vec<AxmlEvent>> {
        events
            .iter()
            .map(|event| match event {
                AxmlEvent::StartElement {
                    namespace,
                    name,
                    attributes,
                } => {
                    let namespace = self.adopt_namespace(source, *namespace)?;
                    let name = self.adopt_string(source, *name);
                    let mut adopted = attributes
                        .iter()
                        .map(|attr| self.adopt_attribute(source, attr, resources))
                        .collect::<Result<Vec<_>>>()?;
                    self.sort_attributes(&mut adopted);
                    Ok(AxmlEvent::StartElement {
                        namespace,
                        name,
                        attributes: adopted,
                    })
                }
                AxmlEvent::EndElement { namespace, name } => Ok(AxmlEvent::EndElement {
                    namespace: self.adopt_namespace(source, *namespace)?,
                    name: self.adopt_string(source, *name),
                }),
                AxmlEvent::StartNamespace { .. } | AxmlEvent::EndNamespace { .. } => Err(invalid(
                    "axml adopt",
                    "a namespace declaration is not part of an element subtree",
                )),
            })
            .collect()
    }

    fn adopt_string(&mut self, source: &AxmlDocument, index: u32) -> u32 {
        let value = source.string(index).unwrap_or_default().into_owned();
        self.intern_string(&value)
    }

    fn adopt_namespace(
        &mut self,
        source: &AxmlDocument,
        namespace: Option<u32>,
    ) -> Result<Option<u32>> {
        let Some(uri) = namespace.and_then(|index| source.string(index)) else {
            return Ok(None);
        };
        self.namespace_index(&uri).map(Some).ok_or_else(|| {
            invalid(
                "axml adopt",
                format!(
                    "the document declares no namespace {uri}; declare it with declareNamespace(prefix, \"{uri}\")"
                ),
            )
        })
    }

    fn adopt_attribute(
        &mut self,
        source: &AxmlDocument,
        attr: &AxmlAttribute,
        resources: Option<&ResourceTable>,
    ) -> Result<AxmlAttribute> {
        let uri = attr
            .namespace
            .and_then(|index| source.string(index))
            .map(std::borrow::Cow::into_owned);
        let local = source.string(attr.name).unwrap_or_default().into_owned();
        let (namespace, name) = self.bind_attribute(uri.as_deref(), &local, resources)?;
        let value = match attr.value.string_index() {
            Some(index) => ResValue::string(self.adopt_string(source, index)),
            None => attr.value,
        };
        let mut adopted = AxmlAttribute::new(namespace, name, value);
        adopted.raw_value = attr.raw_value.map(|index| self.adopt_string(source, index));
        Ok(adopted)
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
        mut attributes: Vec<AxmlAttribute>,
    ) {
        self.sort_attributes(&mut attributes);
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
