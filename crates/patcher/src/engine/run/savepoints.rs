// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_journal::Savepoint;

use crate::context::PatchContext;
use crate::error::Result;

/// The savepoints a run holds, each opened before its patch executed. A patch
/// the run may still have to take back keeps its savepoint to the end: a
/// dependency, which a plan can stop needing, and a patch that finalizes, whose
/// finalizer can fail after its dependents ran.
pub(super) struct Savepoints {
    next: Savepoint,
    held: Vec<(usize, Savepoint)>,
}

impl Default for Savepoints {
    fn default() -> Self {
        Self {
            next: Savepoint::FIRST,
            held: Vec::new(),
        }
    }
}

impl Savepoints {
    pub(super) fn open(&mut self, ctx: &mut PatchContext<'_>, idx: usize) -> Result<Savepoint> {
        let savepoint = self.next;
        self.next = savepoint.next();
        ctx.open_savepoint(savepoint);
        #[cfg(feature = "bridge")]
        crate::bridge::savepoints::open_savepoint(savepoint)?;
        self.held.push((idx, savepoint));
        Ok(savepoint)
    }

    pub(super) fn release(
        &mut self,
        ctx: &mut PatchContext<'_>,
        savepoint: Savepoint,
    ) -> Result<()> {
        ctx.release_savepoint(savepoint);
        #[cfg(feature = "bridge")]
        crate::bridge::savepoints::release_savepoint(savepoint)?;
        self.held.retain(|&(_, held)| held != savepoint);
        Ok(())
    }

    pub(super) fn rollback(
        &mut self,
        ctx: &mut PatchContext<'_>,
        savepoint: Savepoint,
    ) -> Result<()> {
        ctx.rollback_to(savepoint);
        #[cfg(feature = "bridge")]
        crate::bridge::savepoints::rollback_to(savepoint)?;
        self.held.retain(|&(_, held)| held < savepoint);
        Ok(())
    }

    pub(super) fn earliest(
        &self,
        taken_back: impl Fn(usize) -> bool,
    ) -> Option<(usize, Savepoint)> {
        self.held.iter().copied().find(|&(idx, _)| taken_back(idx))
    }

    /// Keeps all the run's work, oldest savepoint first so nothing is kept for
    /// a savepoint about to close.
    pub(super) fn release_all(&mut self, ctx: &mut PatchContext<'_>) -> Result<()> {
        for (_, savepoint) in std::mem::take(&mut self.held) {
            ctx.release_savepoint(savepoint);
            #[cfg(feature = "bridge")]
            crate::bridge::savepoints::release_savepoint(savepoint)?;
        }
        Ok(())
    }
}
