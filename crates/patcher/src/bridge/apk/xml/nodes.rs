// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::axml::{self, AxmlAttribute, AxmlDocument, AxmlEvent};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::bridge) enum Tree {
    Document,
    Detached(ElementId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::bridge) struct ElementId(pub(in crate::bridge) u32);

impl std::fmt::Display for ElementId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::bridge) struct Position {
    pub(in crate::bridge) tree: Tree,
    pub(in crate::bridge) index: usize,
}

#[derive(Default)]
pub(in crate::bridge) struct Nodes {
    pub(in crate::bridge) positions: Vec<Option<Position>>,
    pub(in crate::bridge) ids: HashMap<Position, ElementId>,
    pub(in crate::bridge) detached: HashMap<ElementId, Vec<AxmlEvent>>,
}

impl Nodes {
    pub(in crate::bridge) fn id(&mut self, position: Position) -> Result<ElementId, String> {
        if let Some(&id) = self.ids.get(&position) {
            return Ok(id);
        }
        let id =
            u32::try_from(self.positions.len()).map_err(|e| format!("XML element table: {e}"))?;
        let id = ElementId(id);
        self.positions.push(Some(position));
        self.ids.insert(position, id);
        Ok(id)
    }

    pub(in crate::bridge) fn position(&self, id: ElementId) -> Result<Position, String> {
        self.positions
            .get(id.0 as usize)
            .copied()
            .flatten()
            .ok_or_else(|| format!("invalid or removed XML element {id}"))
    }

    pub(in crate::bridge) fn reindex(&mut self) {
        self.ids = self
            .positions
            .iter()
            .enumerate()
            .filter_map(|(id, position)| position.map(|position| (position, ElementId(id as u32))))
            .collect();
    }

    pub(in crate::bridge) fn inserted(&mut self, tree: Tree, at: usize, count: usize) {
        for position in self
            .positions
            .iter_mut()
            .flatten()
            .filter(|position| position.tree == tree && position.index >= at)
        {
            position.index += count;
        }
        self.reindex();
    }

    pub(in crate::bridge) fn inserted_document(&mut self, at: usize, count: usize) {
        self.inserted(Tree::Document, at, count);
    }

    pub(in crate::bridge) fn events<'a>(
        &'a self,
        document: &'a AxmlDocument,
        tree: Tree,
    ) -> Result<&'a [AxmlEvent], String> {
        match tree {
            Tree::Document => Ok(document.events()),
            Tree::Detached(id) => self
                .detached
                .get(&id)
                .map(Vec::as_slice)
                .ok_or_else(|| format!("missing detached XML tree {id}")),
        }
    }

    pub(in crate::bridge) fn end(
        &self,
        document: &AxmlDocument,
        position: Position,
    ) -> Result<usize, String> {
        axml::element_end(self.events(document, position.tree)?, position.index)
            .ok_or_else(|| "XML handle does not name a terminated element".into())
    }

    pub(in crate::bridge) fn pending(
        &mut self,
        events: Vec<AxmlEvent>,
    ) -> Result<ElementId, String> {
        let id =
            u32::try_from(self.positions.len()).map_err(|e| format!("XML element table: {e}"))?;
        let id = ElementId(id);
        self.id(Position {
            tree: Tree::Detached(id),
            index: 0,
        })?;
        self.detached.insert(id, events);
        Ok(id)
    }

    pub(in crate::bridge) fn detach(
        &mut self,
        document: &mut AxmlDocument,
        element: ElementId,
    ) -> Result<(), String> {
        let position = self.position(element)?;
        if position
            == (Position {
                tree: Tree::Detached(element),
                index: 0,
            })
        {
            return Ok(());
        }
        let end = self.end(document, position)?;
        let events = match position.tree {
            Tree::Document => document
                .detach_element(position.index)
                .map_err(|e| e.to_string())?,
            Tree::Detached(id) => self
                .detached
                .get_mut(&id)
                .expect("tree checked by end")
                .drain(position.index..=end)
                .collect(),
        };
        let count = events.len();
        for held in self
            .positions
            .iter_mut()
            .flatten()
            .filter(|held| held.tree == position.tree)
        {
            if (position.index..=end).contains(&held.index) {
                *held = Position {
                    tree: Tree::Detached(element),
                    index: held.index - position.index,
                };
            } else if held.index > end {
                held.index -= count;
            }
        }
        self.detached.insert(element, events);
        self.reindex();
        Ok(())
    }

    pub(in crate::bridge) fn attach(
        &mut self,
        document: &mut AxmlDocument,
        element: ElementId,
        target: Position,
    ) -> Result<(), String> {
        let events = self
            .detached
            .remove(&element)
            .ok_or_else(|| format!("element {element} is not detached"))?;
        let count = events.len();
        self.inserted(target.tree, target.index, count);
        match target.tree {
            Tree::Document => document
                .insert_events(target.index, events)
                .map_err(|e| e.to_string())?,
            Tree::Detached(id) => {
                self.detached
                    .get_mut(&id)
                    .expect("target tree validated before detach")
                    .splice(target.index..target.index, events);
            }
        }
        for position in self
            .positions
            .iter_mut()
            .flatten()
            .filter(|position| position.tree == Tree::Detached(element))
        {
            *position = Position {
                tree: target.tree,
                index: target.index + position.index,
            };
        }
        self.reindex();
        Ok(())
    }

    pub(in crate::bridge) fn attributes<'a>(
        &'a self,
        document: &'a AxmlDocument,
        element: ElementId,
    ) -> Result<&'a [AxmlAttribute], String> {
        let position = self.position(element)?;
        match self.events(document, position.tree)?.get(position.index) {
            Some(AxmlEvent::StartElement { attributes, .. }) => Ok(attributes),
            _ => Err(format!("element {element} is absent")),
        }
    }

    pub(in crate::bridge) fn edit_attributes<R>(
        &mut self,
        document: &mut AxmlDocument,
        element: ElementId,
        f: impl FnOnce(&mut AxmlDocument, &mut Vec<AxmlAttribute>) -> Result<R, String>,
    ) -> Result<R, String> {
        let position = self.position(element)?;
        match position.tree {
            Tree::Document => document
                .edit_attributes(position.index, f)
                .ok_or_else(|| format!("element {element} is absent"))?,
            Tree::Detached(id) => {
                let Some(AxmlEvent::StartElement { attributes, .. }) = self
                    .detached
                    .get_mut(&id)
                    .and_then(|events| events.get_mut(position.index))
                else {
                    return Err(format!("element {element} is absent"));
                };
                let mut pending = std::mem::take(attributes);
                let result = f(document, &mut pending);
                *attributes = pending;
                result
            }
        }
    }
}
