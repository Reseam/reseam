// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};

use reseam_apk::DexSet;
use reseam_model::analysis::ClassRef;

/// Where a class is defined and what it extends and implements.
pub(super) struct ClassInfo {
    pub(super) dex: usize,
    pub(super) index: usize,
    pub(super) superclass: Option<ClassRef>,
    pub(super) interfaces: Vec<ClassRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Inheritance {
    Extends,
    Implements,
}

/// Every class the APK defines, and its direct subclasses and implementers.
/// A class defined twice is the first definition, the one Android loads.
pub(super) struct Hierarchy {
    classes: HashMap<ClassRef, ClassInfo>,
    children: HashMap<ClassRef, Vec<(ClassRef, Inheritance)>>,
}

impl Hierarchy {
    pub(super) fn build(dex: &DexSet) -> Self {
        let mut classes = HashMap::new();
        for (dex_idx, file) in dex.iter().enumerate() {
            let class_ref = |type_| class_ref(&file.type_descriptor(type_));
            for (index, header) in file.classes().headers().enumerate() {
                classes
                    .entry(class_ref(header.class_type))
                    .or_insert_with(|| ClassInfo {
                        dex: dex_idx,
                        index,
                        superclass: header.superclass.map(class_ref),
                        interfaces: file
                            .classes()
                            .interfaces(index)
                            .into_iter()
                            .map(class_ref)
                            .collect(),
                    });
            }
        }
        let mut children: HashMap<ClassRef, Vec<(ClassRef, Inheritance)>> = HashMap::new();
        for (class, info) in &classes {
            let parents = info
                .superclass
                .iter()
                .map(|parent| (parent, Inheritance::Extends))
                .chain(
                    info.interfaces
                        .iter()
                        .map(|parent| (parent, Inheritance::Implements)),
                );
            for (parent, inheritance) in parents {
                children
                    .entry(parent.clone())
                    .or_default()
                    .push((class.clone(), inheritance));
            }
        }
        for list in children.values_mut() {
            list.sort();
        }
        Self { classes, children }
    }

    pub(super) fn get(&self, class: &ClassRef) -> Option<&ClassInfo> {
        self.classes.get(class)
    }

    pub(super) fn children(&self, class: &ClassRef) -> &[(ClassRef, Inheritance)] {
        self.children.get(class).map_or(&[], Vec::as_slice)
    }

    /// Every class that extends or implements `class`, directly or not.
    pub(super) fn descendants(&self, class: &ClassRef) -> Vec<ClassRef> {
        let mut seen = HashSet::new();
        let mut pending = vec![class.clone()];
        while let Some(class) = pending.pop() {
            for (child, _) in self.children(&class) {
                if seen.insert(child.clone()) {
                    pending.push(child.clone());
                }
            }
        }
        let mut descendants: Vec<ClassRef> = seen.into_iter().collect();
        descendants.sort();
        descendants
    }

    /// The supertypes of `class` the APK does not define, other than
    /// `java.lang.Object`: the types through which the system can call it.
    pub(super) fn external_supertypes(&self, class: &ClassRef) -> Vec<ClassRef> {
        let mut external = Vec::new();
        let mut seen = HashSet::new();
        let mut pending = vec![class.clone()];
        while let Some(class) = pending.pop() {
            if !seen.insert(class.clone()) {
                continue;
            }
            match self.classes.get(&class) {
                Some(info) => {
                    pending.extend(info.superclass.iter().cloned());
                    pending.extend(info.interfaces.iter().cloned());
                }
                None if class.descriptor() != "Ljava/lang/Object;" => external.push(class),
                None => {}
            }
        }
        external.sort();
        external
    }

    pub(super) fn classes(&self) -> impl Iterator<Item = (&ClassRef, &ClassInfo)> {
        self.classes.iter()
    }
}

pub(super) fn class_ref(descriptor: &str) -> ClassRef {
    descriptor
        .parse()
        .expect("the DEX writes valid class descriptors")
}
