// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The mutation revision patch runtimes check their caches against: which
//! methods changed since a revision, or that everything may have.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

const JOURNAL_LEN: usize = 1024;

thread_local! {
    static REVISION: Cell<u64> = const { Cell::new(0) };
    static CHANGES: RefCell<VecDeque<MethodChange>> = const { RefCell::new(VecDeque::new()) };
    static STRUCTURAL_REVISION: Cell<u64> = const { Cell::new(0) };
}

struct MethodChange {
    revision: u64,
    handle: u32,
}

pub(super) fn reset() {
    REVISION.with(|revision| revision.set(0));
    STRUCTURAL_REVISION.with(|revision| revision.set(0));
    CHANGES.with(|changes| changes.borrow_mut().clear());
}

/// Records a change that may affect any cached data.
pub(super) fn changed() {
    REVISION.with(|revision| revision.set(revision.get().wrapping_add(1)));
    STRUCTURAL_REVISION.with(|revision| revision.set(mutation_revision()));
    CHANGES.with(|changes| changes.borrow_mut().clear());
}

pub(super) fn method_changed(handle: u32) {
    REVISION.with(|revision| revision.set(revision.get().wrapping_add(1)));
    CHANGES.with(|changes| {
        let mut changes = changes.borrow_mut();
        if changes.len() == JOURNAL_LEN {
            let removed = changes.pop_front().expect("change journal is full");
            STRUCTURAL_REVISION.with(|revision| revision.set(removed.revision));
        }
        changes.push_back(MethodChange {
            revision: mutation_revision(),
            handle,
        });
    });
}

/// Returns changes since a reader's revision. Readers older than the bounded
/// journal receive `None` and must discard all cached data, just as for a
/// structural mutation. `Some` lists only the methods that changed.
#[boltffi::export]
pub fn mutation_changes(since: u64) -> Option<Vec<u32>> {
    if since > mutation_revision() || STRUCTURAL_REVISION.with(|revision| since < revision.get()) {
        return None;
    }
    let mut handles: Vec<_> = CHANGES.with(|changes| {
        changes
            .borrow()
            .iter()
            .rev()
            .take_while(|change| change.revision > since)
            .map(|change| change.handle)
            .collect()
    });
    handles.sort_unstable();
    handles.dedup();
    Some(handles)
}

#[boltffi::export]
pub fn mutation_revision() -> u64 {
    REVISION.with(Cell::get)
}
