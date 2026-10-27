// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cell::RefCell;

use reseam_journal::{Journal, Savepoint};
use rustc_hash::FxHashMap;

use reseam_dex::DexFile;

use super::convert::invalid;
use super::types::PoolOrigin;

thread_local! {
    static POOL_COPIES: RefCell<PoolCopies> = RefCell::new(PoolCopies::default());
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::bridge) enum PoolKind {
    Handle,
    CallSite,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PoolCopy {
    origin: PoolOrigin,
    destination: u32,
    kind: PoolKind,
}

/// Where call sites and method handles were already copied, so each is copied once.
#[derive(Default)]
struct PoolCopies {
    copies: FxHashMap<PoolCopy, u32>,
    journal: Journal<PoolCopy, (PoolCopy, Option<u32>)>,
}

fn copied_pool(origin: PoolOrigin, destination: u32, kind: PoolKind) -> Option<u32> {
    POOL_COPIES.with(|copies| {
        copies
            .borrow()
            .copies
            .get(&PoolCopy {
                origin,
                destination,
                kind,
            })
            .copied()
    })
}

fn remember_pool(origin: PoolOrigin, destination: u32, kind: PoolKind, index: u32) {
    let copy = PoolCopy {
        origin,
        destination,
        kind,
    };
    POOL_COPIES.with(|copies| {
        let copies = &mut *copies.borrow_mut();
        let previous = copies.copies.insert(copy, index);
        copies.journal.record(copy, || (copy, previous));
    });
}

pub(in crate::bridge) fn open_savepoint(savepoint: Savepoint) {
    POOL_COPIES.with(|copies| copies.borrow_mut().journal.open(savepoint));
}

pub(in crate::bridge) fn release_savepoint(savepoint: Savepoint) {
    POOL_COPIES.with(|copies| copies.borrow_mut().journal.release(savepoint));
}

pub(in crate::bridge) fn rollback_to(savepoint: Savepoint) {
    POOL_COPIES.with(|copies| {
        let copies = &mut *copies.borrow_mut();
        for (copy, previous) in copies.journal.rollback(savepoint) {
            match previous {
                Some(index) => copies.copies.insert(copy, index),
                None => copies.copies.remove(&copy),
            };
        }
    });
}

pub(in crate::bridge) fn reset() {
    POOL_COPIES.with(|copies| *copies.borrow_mut() = PoolCopies::default());
}

// A transient assembly pool has no application identity and never changes run caches.
pub(in crate::bridge) fn pool_origin(
    dex_index: Option<usize>,
    index: u32,
) -> reseam_dex::Result<Option<PoolOrigin>> {
    dex_index
        .map(|dex_index| {
            Ok(PoolOrigin {
                dex_index: u32::try_from(dex_index)
                    .map_err(|_| invalid("DEX index exceeds 32 bits"))?,
                index,
            })
        })
        .transpose()
}

/// Interns an entry imported from `origin` into the DEX at `destination`,
/// comparing it only with where it may already be there: its own index when
/// it stays in its DEX, and where an earlier import copied it. An entry with
/// no origin is compared with every entry.
pub(in crate::bridge) fn intern_copy(
    dex: &mut DexFile,
    origin: Option<PoolOrigin>,
    destination: Option<usize>,
    kind: PoolKind,
    intern: impl FnOnce(&mut DexFile, Option<Vec<u32>>) -> reseam_dex::Result<u32>,
) -> reseam_dex::Result<u32> {
    let destination = destination
        .map(u32::try_from)
        .transpose()
        .map_err(|_| invalid("DEX index exceeds 32 bits"))?;
    let origin = origin.filter(|_| destination.is_some());
    let candidates = origin.zip(destination).map(|(origin, destination)| {
        (origin.dex_index == destination)
            .then_some(origin.index)
            .into_iter()
            .chain(copied_pool(origin, destination, kind))
            .collect()
    });
    let index = intern(dex, candidates)?;
    if let Some((origin, destination)) = origin.zip(destination) {
        remember_pool(origin, destination, kind, index);
    }
    Ok(index)
}
