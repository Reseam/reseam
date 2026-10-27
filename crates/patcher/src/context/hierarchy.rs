// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use reseam_dex::DexFile;

use super::PatchContext;

const OBJECT: &str = "Ljava/lang/Object;";

fn is_integral(descriptor: &str) -> bool {
    matches!(descriptor, "Z" | "B" | "S" | "C" | "I")
}

fn is_reference(descriptor: &str) -> bool {
    descriptor.starts_with(['L', '['])
}

impl PatchContext<'_> {
    /// Whether a value of type `actual` may be stored where `expected` is declared. A class
    /// whose ancestry leaves the app and its extensions cannot be proven unrelated, so it passes.
    pub fn is_assignable(&self, actual: &str, expected: &str) -> bool {
        if actual == expected || is_integral(actual) && is_integral(expected) {
            return true;
        }
        if !is_reference(actual) || !is_reference(expected) {
            return false;
        }
        if expected == OBJECT {
            return true;
        }
        if let Some(element) = actual.strip_prefix('[') {
            return match expected.strip_prefix('[') {
                Some(destination) if is_reference(element) => {
                    self.is_assignable(element, destination)
                }
                Some(destination) => element == destination,
                None => matches!(expected, "Ljava/lang/Cloneable;" | "Ljava/io/Serializable;"),
            };
        }
        if expected.starts_with('[') {
            return false;
        }
        let Some(ancestors) = self.ancestors(actual) else {
            return true;
        };
        ancestors.contains(expected) || self.ancestors(expected).is_none()
    }

    /// `class` and every class and interface it extends or implements; `None` once the walk
    /// reaches a class neither the app nor its extensions define.
    fn ancestors(&self, class: &str) -> Option<HashSet<String>> {
        let mut ancestors = HashSet::new();
        let mut pending = vec![class.to_owned()];
        while let Some(class) = pending.pop() {
            if !ancestors.insert(class.clone()) || class == OBJECT {
                continue;
            }
            let (dex, index) = self.class_definition(&class)?;
            let descriptor = |ty| dex.type_descriptor(ty).into_owned();
            pending.extend(dex.classes().header(index).superclass.map(descriptor));
            pending.extend(dex.classes().interfaces(index).into_iter().map(descriptor));
        }
        Some(ancestors)
    }

    fn class_definition(&self, descriptor: &str) -> Option<(&DexFile, usize)> {
        match self.find_class(descriptor) {
            Some(location) => Some((self.dex_file(location.dex_idx)?, location.class_idx)),
            None => self.extensions.unlinked_class(descriptor),
        }
    }
}
