// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where resource ids are used in compiled resource XML: layouts, menus,
//! drawables and the rest under `res/`.

use std::collections::HashMap;

use reseam_apk::axml::{AxmlEvent, android_attr_res_id, is_compiled_axml};
use reseam_apk::{AxmlDocument, ResValue};

use super::Analysis;
use crate::error::Result;

/// A compiled XML file and the resource the table files it under.
pub(super) struct XmlFile {
    pub(super) path: String,
    pub(super) resource: Option<u32>,
}

/// An element with at least one resource reference.
pub(super) struct XmlElement {
    pub(super) file: usize,
    pub(super) tag: String,
    /// Its `android:id`.
    pub(super) view_id: Option<u32>,
}

pub(super) struct XmlIndex {
    pub(super) files: Vec<XmlFile>,
    pub(super) elements: Vec<XmlElement>,
    by_id: HashMap<u32, Vec<usize>>,
}

impl XmlIndex {
    /// The elements that reference resource `id`, its `android:id` included.
    pub(super) fn uses(&self, id: u32) -> impl Iterator<Item = &XmlElement> {
        self.by_id
            .get(&id)
            .into_iter()
            .flatten()
            .map(|&element| &self.elements[element])
    }
}

impl Analysis {
    /// The XML index, read from every component on first use.
    pub(super) fn xml_index(&mut self) -> Result<&XmlIndex> {
        if self.xml.is_none() {
            let index = self.build_xml_index()?;
            self.xml = Some(index);
        }
        Ok(self.xml.as_ref().expect("the index was just built"))
    }

    fn build_xml_index(&mut self) -> Result<XmlIndex> {
        let resources = self.file_resources()?;
        let android_id = android_attr_res_id("id").expect("android:id is a framework attribute");
        let mut index = XmlIndex {
            files: Vec::new(),
            elements: Vec::new(),
            by_id: HashMap::new(),
        };
        for component in 0..self.opened.apk.components().len() {
            let names: Vec<String> = self.opened.apk.components()[component]
                .entry_names()
                .into_iter()
                .filter(|name| {
                    name.starts_with("res/")
                        && std::path::Path::new(name)
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
                })
                .collect();
            for name in names {
                let bytes = self
                    .opened
                    .apk
                    .read_component_entry(component, &name)
                    .map_err(|error| self.opened.error(error))?
                    .expect("the entry was just listed");
                if !is_compiled_axml(&bytes) {
                    continue;
                }
                let document =
                    AxmlDocument::parse(&bytes).map_err(|error| self.opened.error(error))?;
                let file = index.files.len();
                index.add(file, &document, android_id);
                index.files.push(XmlFile {
                    resource: resources.get(&name).copied(),
                    path: name,
                });
            }
        }
        Ok(index)
    }

    /// The resource each file path is filed under: a file resource is a string
    /// entry holding its path.
    fn file_resources(&self) -> Result<HashMap<String, u32>> {
        let mut resources = HashMap::new();
        self.for_each_string(|_, path, uses| {
            if path.starts_with("res/")
                && let Some(used) = uses.first()
            {
                resources.entry(path.to_owned()).or_insert(used.res_id);
            }
            Ok(())
        })?;
        Ok(resources)
    }
}

impl XmlIndex {
    fn add(&mut self, file: usize, document: &AxmlDocument, android_id: u32) {
        for (event, element) in document.events().iter().enumerate() {
            let AxmlEvent::StartElement { attributes, .. } = element else {
                continue;
            };
            let references: Vec<(u32, bool)> = attributes
                .iter()
                .filter(|attribute| {
                    attribute.value().kind == ResValue::REFERENCE && attribute.value().data != 0
                })
                .map(|attribute| {
                    let is_id = document.resource_id_for(attribute.name()) == Some(android_id);
                    (attribute.value().data, is_id)
                })
                .collect();
            if references.is_empty() {
                continue;
            }
            let element = self.elements.len();
            self.elements.push(XmlElement {
                file,
                tag: document
                    .element_name(event)
                    .expect("a start element has a name")
                    .into_owned(),
                view_id: references
                    .iter()
                    .find(|(_, is_id)| *is_id)
                    .map(|(id, _)| *id),
            });
            for (id, _) in references {
                let uses = self.by_id.entry(id).or_default();
                if uses.last() != Some(&element) {
                    uses.push(element);
                }
            }
        }
    }
}
