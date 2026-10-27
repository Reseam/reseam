// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::{Mutex, PoisonError};

use rustc_hash::FxHashMap;

use crate::model::TypeIdx;

/// Remembered descriptor lookups. Type ids are only ever appended, so a found
/// index stays valid, and a miss holds until the type table grows.
#[derive(Debug, Default)]
pub(crate) struct TypeLookups(Mutex<FxHashMap<Box<str>, Lookup>>);

#[derive(Debug, Clone, Copy)]
enum Lookup {
    Found(TypeIdx),
    Missing { types: usize },
}

impl TypeLookups {
    /// The remembered result for `descriptor` while the table holds `types`
    /// entries, or `find`'s, which is then remembered.
    pub(crate) fn get_or_find(
        &self,
        descriptor: &str,
        types: usize,
        find: impl FnOnce() -> Option<TypeIdx>,
    ) -> Option<TypeIdx> {
        match self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(descriptor)
        {
            Some(Lookup::Found(idx)) => return Some(*idx),
            Some(Lookup::Missing { types: seen }) if *seen == types => return None,
            _ => {}
        }
        let found = find();
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                descriptor.into(),
                found.map_or(Lookup::Missing { types }, Lookup::Found),
            );
        found
    }

    pub(crate) fn clear(&mut self) {
        self.0
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

impl Clone for TypeLookups {
    fn clone(&self) -> Self {
        Self::default()
    }
}
