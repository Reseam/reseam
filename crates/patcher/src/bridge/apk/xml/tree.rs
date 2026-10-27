// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bridge::run_state::checked;
use boltffi::export;
use reseam_apk::ResourceScope;
use reseam_apk::axml::{AxmlEvent, NodeMetadata};

use super::documents::{with_edit, with_read};
use super::nodes::{ElementId, Position, Tree};

#[export]
pub fn xml_create_element(doc: u32, tag: String) -> u32 {
    checked(
        with_edit(doc, |nodes, document, _| {
            let name = document.intern_string(&tag);
            nodes.pending(vec![
                AxmlEvent::StartElement {
                    metadata: NodeMetadata::default(),
                    namespace: None,
                    name,
                    attributes: Vec::new(),
                },
                AxmlEvent::EndElement {
                    metadata: NodeMetadata::default(),
                    namespace: None,
                    name,
                },
            ])
        })
        .map(|id| id.0),
    )
}

#[export]
pub fn xml_append_child(doc: u32, parent: u32, child: u32) -> Result<(), String> {
    let parent = ElementId(parent);
    let child = ElementId(child);
    with_edit(doc, |nodes, document, _| {
        let parent_position = nodes.position(parent)?;
        let child_position = nodes.position(child)?;
        let child_end = nodes.end(document, child_position)?;
        nodes.end(document, parent_position)?;
        if parent_position.tree == child_position.tree
            && (child_position.index..=child_end).contains(&parent_position.index)
        {
            return Err("an XML element cannot contain itself".into());
        }
        nodes.detach(document, child)?;
        let parent_position = nodes.position(parent)?;
        let index = nodes.end(document, parent_position)?;
        nodes.attach(
            document,
            child,
            Position {
                tree: parent_position.tree,
                index,
            },
        )
    })
}

#[export]
pub fn xml_insert_before(doc: u32, child: u32, before: u32) -> Result<u32, String> {
    let child = ElementId(child);
    let before = ElementId(before);
    with_edit(doc, |nodes, document, _| {
        let target = nodes.position(before)?;
        if target.tree != Tree::Document {
            return Err("insertBefore requires an attached anchor".into());
        }
        let child_position = nodes.position(child)?;
        let end = nodes.end(document, child_position)?;
        if child_position.tree == target.tree
            && (child_position.index..=end).contains(&target.index)
        {
            return Err("an XML element cannot be inserted into itself".into());
        }
        nodes.detach(document, child)?;
        nodes.attach(document, child, nodes.position(before)?)?;
        Ok(child.0)
    })
}

#[export]
pub fn xml_remove_element(doc: u32, element: u32) {
    let element = ElementId(element);
    checked(with_edit(doc, |nodes, document, _| {
        nodes.detach(document, element)?;
        nodes.detached.remove(&element);
        for position in &mut nodes.positions {
            if position.is_some_and(|position| position.tree == Tree::Detached(element)) {
                *position = None;
            }
        }
        nodes.reindex();
        Ok(())
    }));
}

#[export]
pub fn xml_adopt(doc: u32, source: u32, element: u32) -> Result<u32, String> {
    let element = ElementId(element);
    if source == doc {
        return Err("use clone within one XML document".into());
    }
    let (strings, subtree) = with_read(source, |nodes, document| {
        let position = nodes.position(element)?;
        let end = nodes.end(document, position)?;
        // StringPool clones share the input backing; only the requested subtree is copied.
        Ok((
            document.string_pool().clone(),
            nodes.events(document, position.tree)?[position.index..=end].to_vec(),
        ))
    })?;
    with_edit(doc, |nodes, document, ctx| {
        let events = ctx
            .apk_mut()
            .with_resource_scope(0, |scope| {
                document.adopt(
                    &strings,
                    &subtree,
                    scope.as_deref().map(ResourceScope::table),
                )
            })
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        nodes.pending(events)
    })
    .map(|id| id.0)
}

/// Whether a cloned element takes its children along.
#[boltffi::data]
#[derive(Debug, Clone, Copy)]
pub enum CloneDepth {
    Shallow,
    Deep,
}

#[export]
pub fn xml_clone_element(doc: u32, element: u32, depth: CloneDepth) -> u32 {
    let element = ElementId(element);
    checked(
        with_read(doc, |nodes, document| {
            let position = nodes.position(element)?;
            let events = nodes.events(document, position.tree)?;
            let end = nodes.end(document, position)?;
            let events = match depth {
                CloneDepth::Deep => events[position.index..=end].to_vec(),
                CloneDepth::Shallow => vec![events[position.index].clone(), events[end].clone()],
            };
            nodes.pending(events)
        })
        .map(|id| id.0),
    )
}
