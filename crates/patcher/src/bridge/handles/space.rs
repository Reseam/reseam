// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::{PatcherError, Result as PatcherResult};

const HANDLE_BLOCK: usize = 4096;
static NEXT_HANDLE_BLOCK: AtomicU32 = AtomicU32::new(0);

/// Maps table slots to handle numbers unique across every table in the
/// process, so a handle from one table is never valid in another.
#[derive(Default)]
pub(crate) struct HandleSpace {
    ranges: Vec<u32>,
}

impl HandleSpace {
    pub fn allocate(&mut self, slot: usize) -> PatcherResult<u32> {
        if slot.is_multiple_of(HANDLE_BLOCK) {
            let first = NEXT_HANDLE_BLOCK
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                    next.checked_add(HANDLE_BLOCK as u32)
                })
                .map_err(|_| {
                    PatcherError::Bridge("native handle identities are exhausted".into())
                })?;
            self.ranges.push(first);
        }
        Ok(self.handle(slot))
    }

    pub fn handle(&self, slot: usize) -> u32 {
        self.ranges[slot / HANDLE_BLOCK] + (slot % HANDLE_BLOCK) as u32
    }

    pub fn slot(&self, handle: u32) -> Option<usize> {
        let range = self
            .ranges
            .partition_point(|first| *first <= handle)
            .checked_sub(1)?;
        let offset = handle - self.ranges[range];
        (offset < HANDLE_BLOCK as u32).then_some(range * HANDLE_BLOCK + offset as usize)
    }
}
