// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use super::android_attrs::android_attr_res_id;
use super::{ANDROID_NS, AxmlAttribute, AxmlDocument, AxmlEvent};

impl AxmlDocument {
    pub fn root(&self) -> Option<usize> {
        self.elements
            .iter()
            .position(|event| matches!(event, AxmlEvent::StartElement { .. }))
    }

    pub fn element_name(&self, index: usize) -> Option<Cow<'_, str>> {
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

    pub fn attributes(&self, index: usize) -> &[AxmlAttribute] {
        match self.elements.get(index) {
            Some(AxmlEvent::StartElement { attributes, .. }) => attributes,
            _ => &[],
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

    /// Whether `attr` is `local` in `namespace`. An `android:` attribute is the framework attribute
    /// with that resource ID whatever its name string holds, as the platform reads it; protected
    /// manifests blank or scramble those strings.
    pub fn is_attribute(&self, attr: &AxmlAttribute, namespace: Option<u32>, local: &str) -> bool {
        let framework_id = namespace
            .filter(|&uri| self.string(uri).as_deref() == Some(ANDROID_NS))
            .and_then(|_| android_attr_res_id(local));
        match framework_id {
            Some(id) if self.resource_id_for(attr.name) == Some(id) => true,
            _ => attr.namespace == namespace && self.string(attr.name).as_deref() == Some(local),
        }
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
}
