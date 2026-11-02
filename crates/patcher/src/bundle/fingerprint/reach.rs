// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What one patch reaches in the [`Graph`], and its hash. Reaching a member
//! reaches its class header, which brings what [`Class::implicit`] lists. An
//! extension class a string names (bindings name extension classes that way,
//! and find their members by name) counts whole.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use reseam_dex::{References, class_descriptor};
use sha2::{Digest, Sha256};

use super::graph::{Class, Graph, Side};
use crate::bundle::index::{Declaration, MemberKind};

/// The hash of everything the patch that `roots` declare reaches.
pub(super) fn fingerprint(graph: &Graph, roots: &[&Declaration]) -> [u8; 32] {
    let mut reach = Reach {
        graph,
        pending: Vec::new(),
        headers: BTreeSet::new(),
        members: BTreeSet::new(),
        strings: BTreeSet::new(),
    };
    for root in roots {
        let owner = class_descriptor(&root.owner);
        if let Some(class) = graph.patch.get(&owner) {
            let members = match root.kind {
                MemberKind::Field => &class.fields,
                MemberKind::Method => &class.methods,
            };
            for key in members.keys().filter(|key| member_name(key) == root.member) {
                reach
                    .pending
                    .push(Item::Member(Side::Patch, owner.clone(), key.clone()));
            }
        }
        reach.pending.push(Item::Header(Side::Patch, owner));
        reach.pending.push(Item::Header(
            Side::Patch,
            class_descriptor(&root.class_name),
        ));
    }
    reach.run();
    reach.hash()
}

enum Item {
    Header(Side, String),
    Member(Side, String, String),
    /// An extension class, with every member.
    Whole(String),
}

struct Reach<'g> {
    graph: &'g Graph,
    pending: Vec<Item>,
    headers: BTreeSet<(Side, &'g str)>,
    members: BTreeSet<(Side, &'g str, &'g str)>,
    strings: BTreeSet<&'g str>,
}

impl<'g> Reach<'g> {
    fn run(&mut self) {
        while let Some(item) = self.pending.pop() {
            match item {
                Item::Header(side, descriptor) => self.header(side, &descriptor),
                Item::Member(side, class, key) => self.member(side, &class, &key),
                Item::Whole(descriptor) => self.whole(descriptor),
            }
        }
    }

    fn header(&mut self, side: Side, descriptor: &str) {
        let graph = self.graph;
        let Some((descriptor, class)) = graph.classes(side).get_key_value(descriptor) else {
            return;
        };
        if !self.headers.insert((side, descriptor)) {
            return;
        }
        self.references(side, &class.header.references);
        for supertype in &class.supertypes {
            self.pending.push(Item::Header(side, supertype.clone()));
        }
        for key in class.implicit() {
            self.pending
                .push(Item::Member(side, descriptor.clone(), key.clone()));
        }
    }

    fn member(&mut self, side: Side, descriptor: &str, key: &str) {
        let graph = self.graph;
        let Some((descriptor, class)) = graph.classes(side).get_key_value(descriptor) else {
            return;
        };
        let Some((key, member)) = class.member(key) else {
            return;
        };
        if !self.members.insert((side, descriptor, key)) {
            return;
        }
        self.pending.push(Item::Header(side, descriptor.clone()));
        self.references(side, &member.node.references);
    }

    fn whole(&mut self, descriptor: String) {
        let Some(class) = self.graph.extension.get(&descriptor) else {
            return;
        };
        for key in class.methods.keys().chain(class.fields.keys()) {
            self.pending.push(Item::Member(
                Side::Extension,
                descriptor.clone(),
                key.clone(),
            ));
        }
        self.pending.push(Item::Header(Side::Extension, descriptor));
    }

    fn references(&mut self, side: Side, references: &'g References) {
        for type_ in &references.types {
            self.pending
                .push(Item::Header(side, type_.trim_start_matches('[').to_owned()));
        }
        for descriptor in references.fields.iter().chain(&references.methods) {
            let (class, key) = descriptor
                .split_once("->")
                .expect("member descriptors name their class before `->`");
            self.pending.push(Item::Header(side, class.to_owned()));
            if let Some(owner) = declaring(self.graph.classes(side), class, key) {
                self.pending
                    .push(Item::Member(side, owner.to_owned(), key.to_owned()));
            }
        }
        for string in &references.strings {
            self.strings.insert(string);
            for descriptor in [string.clone(), class_descriptor(string)] {
                if self.graph.extension.contains_key(&descriptor) {
                    self.pending.push(Item::Whole(descriptor));
                }
            }
        }
    }

    /// Hashes the reached declarations, the non-code files of the jars they come
    /// from, and the resources a reached string names: their path, a folder above
    /// them, or their name without extension (how Hermes modules are loaded).
    fn hash(&self) -> [u8; 32] {
        let graph = self.graph;
        let mut hasher = Sha256::new();
        let mut entry = |kind: &str, name: &str, hash: &[u8; 32]| {
            for part in [kind.as_bytes(), name.as_bytes(), hash] {
                hasher.update((part.len() as u64).to_le_bytes());
                hasher.update(part);
            }
        };
        for &(side, descriptor) in &self.headers {
            entry(
                side.name(),
                descriptor,
                &graph.classes(side)[descriptor].header.hash,
            );
        }
        for &(side, descriptor, key) in &self.members {
            let (_, member) = graph.classes(side)[descriptor]
                .member(key)
                .expect("reached members exist");
            entry(
                side.name(),
                &format!("{descriptor}->{key}"),
                &member.node.hash,
            );
        }
        let jars: BTreeSet<usize> = self
            .headers
            .iter()
            .filter_map(|&(side, descriptor)| graph.classes(side)[descriptor].jar)
            .collect();
        for jar in jars {
            for (name, hash) in &graph.jar_files[jar] {
                entry("jar file", name, hash);
            }
        }
        for (path, hash) in &graph.resources {
            let stem = Path::new(path).file_stem().and_then(|stem| stem.to_str());
            let named = self.strings.iter().any(|&string| {
                path == string
                    || path
                        .strip_prefix(string)
                        .is_some_and(|rest| rest.starts_with('/'))
                    || stem == Some(string)
            });
            if named {
                entry("resource", path, hash);
            }
        }
        hasher.finalize().into()
    }
}

/// The class among `classes` that declares `key` for `class`: the class or the nearest supertype.
fn declaring<'c>(
    classes: &'c HashMap<String, Class>,
    class: &'c str,
    key: &str,
) -> Option<&'c str> {
    let mut queue = vec![class];
    let mut seen = BTreeSet::new();
    while let Some(class) = queue.pop() {
        let Some(definition) = classes.get(class) else {
            continue;
        };
        if definition.member(key).is_some() {
            return Some(class);
        }
        queue.extend(
            definition
                .supertypes
                .iter()
                .map(String::as_str)
                .filter(|supertype| seen.insert(*supertype)),
        );
    }
    None
}

/// The name in a member key: `name` of `name:I` or `name(I)V`.
fn member_name(key: &str) -> &str {
    key.split(['(', ':']).next().unwrap_or(key)
}
