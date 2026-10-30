// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::space::HandleSpace;

/// Values the bridge holds for Kotlin between calls, each named by a handle
/// from the moment it is inserted until it is removed. A removed value's
/// handle is never given out again.
pub(crate) struct OwnedHandles<T> {
    identities: HandleSpace,
    slots: Vec<Option<T>>,
}

impl<T> Default for OwnedHandles<T> {
    fn default() -> Self {
        Self {
            identities: HandleSpace::default(),
            slots: Vec::new(),
        }
    }
}

impl<T> OwnedHandles<T> {
    pub fn insert(&mut self, value: T) -> Result<u32, String> {
        let handle = self
            .identities
            .allocate(self.slots.len())
            .map_err(|error| error.to_string())?;
        self.slots.push(Some(value));
        Ok(handle)
    }

    pub fn get(&self, handle: u32) -> Option<&T> {
        self.slots.get(self.identities.slot(handle)?)?.as_ref()
    }

    pub fn get_mut(&mut self, handle: u32) -> Option<&mut T> {
        self.slots.get_mut(self.identities.slot(handle)?)?.as_mut()
    }

    pub fn remove(&mut self, handle: u32) -> Option<T> {
        self.slots.get_mut(self.identities.slot(handle)?)?.take()
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(slot, value)| Some((self.identities.handle(slot), value.as_ref()?)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u32, &mut T)> {
        let identities = &self.identities;
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(slot, value)| Some((identities.handle(slot), value.as_mut()?)))
    }
}
