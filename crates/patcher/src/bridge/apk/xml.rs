// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bridge::run_state::checked;
use boltffi::export;
use reseam_apk::axml::{self, AxmlEvent};

pub(crate) mod attributes;
pub(crate) mod documents;
pub(crate) mod nodes;
pub(crate) mod tree;

use attributes::attribute_text;
use documents::{Source, open, with_edit, with_read};
pub(in crate::bridge) use documents::{edit_manifest, finish, open_manifest, reset};
pub(in crate::bridge) use nodes::Nodes;
use nodes::{ElementId, Position, Tree};

#[export]
pub fn xml_root(doc: u32) -> u32 {
    checked(with_read(doc, |nodes, document| {
        nodes
            .id(Position {
                tree: Tree::Document,
                index: document.root().ok_or("XML document has no root")?,
            })
            .map(|id| id.0)
    }))
}

#[export]
pub fn xml_find_by_tag(doc: u32, tag: String) -> Vec<u32> {
    checked(with_read(doc, |nodes, document| {
        (0..document.events().len())
            .filter(|&index| document.element_name(index).as_deref() == Some(tag.as_str()))
            .map(|index| {
                nodes
                    .id(Position {
                        tree: Tree::Document,
                        index,
                    })
                    .map(|id| id.0)
            })
            .collect()
    }))
}

#[export]
pub fn xml_find_by_attribute(doc: u32, name: String, value: String) -> Vec<u32> {
    checked(with_read(doc, |nodes, document| {
        (0..document.events().len())
            .filter(|&index| {
                attribute_text(document, document.attributes(index), &name).as_deref()
                    == Some(value.as_str())
            })
            .map(|index| {
                nodes
                    .id(Position {
                        tree: Tree::Document,
                        index,
                    })
                    .map(|id| id.0)
            })
            .collect()
    }))
}

#[export]
pub fn xml_children(doc: u32, element: u32) -> Vec<u32> {
    let element = ElementId(element);
    checked(with_read(doc, |nodes, document| {
        let position = nodes.position(element)?;
        nodes.end(document, position)?;
        axml::children(nodes.events(document, position.tree)?, position.index)
            .into_iter()
            .map(|index| {
                nodes
                    .id(Position {
                        tree: position.tree,
                        index,
                    })
                    .map(|id| id.0)
            })
            .collect()
    }))
}

#[export]
pub fn xml_parent(doc: u32, element: u32) -> Option<u32> {
    let element = ElementId(element);
    checked(with_read(doc, |nodes, document| {
        let position = nodes.position(element)?;
        axml::parent(nodes.events(document, position.tree)?, position.index)
            .map(|index| {
                nodes
                    .id(Position {
                        tree: position.tree,
                        index,
                    })
                    .map(|id| id.0)
            })
            .transpose()
    }))
}

#[export]
pub fn xml_tag_name(doc: u32, element: u32) -> String {
    let element = ElementId(element);
    checked(with_read(doc, |nodes, document| {
        let position = nodes.position(element)?;
        let Some(AxmlEvent::StartElement { name, .. }) =
            nodes.events(document, position.tree)?.get(position.index)
        else {
            return Err(format!("element {element} is absent"));
        };
        document
            .string(*name)
            .map(std::borrow::Cow::into_owned)
            .ok_or_else(|| "invalid XML element name".into())
    }))
}

#[export]
pub fn xml_compile(text: String) -> Result<u32, String> {
    let document = crate::bridge::phase::try_with_ctx(|ctx| {
        ctx.apk_mut()
            .with_resource_scope(0, |scope| axml::build_document(&text, scope))
    })
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    open(Source::Memory, Some(document))
}

#[export]
pub fn xml_declare_namespace(doc: u32, prefix: String, uri: String) -> Result<(), String> {
    with_edit(doc, |nodes, document, _| {
        let before = document.events().len();
        document
            .declare_namespace(&prefix, &uri)
            .map_err(|e| e.to_string())?;
        if document.events().len() != before {
            nodes.inserted_document(0, 1);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests;
