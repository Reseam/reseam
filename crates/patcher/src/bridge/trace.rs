// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where the patches of a traced run changed app code, followed through every later edit until
//! patching is over and the run logs each place.

use std::cell::RefCell;
use std::ops::Range;

use reseam_dex::InstructionMap;
use reseam_journal::{Journal, Savepoint};
use rustc_hash::FxHashMap;

use crate::bridge::handles::with_handles;
use crate::context::{Moved, PatchContext, TracePoint};
use crate::error::Result;
use crate::patch::PatchRef;

thread_local! {
    static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) };
}

#[derive(Default)]
struct Trace {
    /// The patch whose phase is running, which the points it adds belong to.
    patch: Option<PatchRef>,
    /// Points by method handle.
    points: FxHashMap<u32, Vec<TracePoint>>,
    journal: Journal<u32, (u32, Vec<TracePoint>)>,
}

impl Trace {
    /// The points of method `handle`, kept for the open savepoint before they change.
    fn points(&mut self, handle: u32) -> &mut Vec<TracePoint> {
        let points = self.points.entry(handle).or_default();
        self.journal.record(handle, || (handle, points.clone()));
        points
    }
}

/// Runs `f` over the run's trace, when the run is traced.
fn with_trace(f: impl FnOnce(&mut Trace)) {
    TRACE.with(|trace| trace.borrow_mut().as_mut().map(f));
}

/// Starts keeping the points of this run.
pub(crate) fn start() {
    TRACE.with(|trace| *trace.borrow_mut() = Some(Trace::default()));
}

pub(super) fn reset() {
    TRACE.with(|trace| *trace.borrow_mut() = None);
}

pub(super) fn begin_patch(patch: &PatchRef) {
    with_trace(|trace| trace.patch = Some(patch.clone()));
}

pub(super) fn end_patch() {
    with_trace(|trace| trace.patch = None);
}

/// Adds a point of the running patch at `index` of method `handle`.
pub(super) fn changed(handle: u32, index: usize) {
    with_trace(|trace| {
        let point = TracePoint {
            patch: trace
                .patch
                .clone()
                .expect("code changes while a patch phase runs"),
            index,
        };
        let points = trace.points(handle);
        if !points.contains(&point) {
            points.push(point);
        }
    });
}

/// Carries the points of method `handle` through an edit that moved its instructions as `map`
/// describes.
pub(super) fn moved(handle: u32, map: &InstructionMap, how: Moved) {
    with_trace(|trace| {
        if trace.points.contains_key(&handle) {
            for point in trace.points(handle) {
                point.relocate(map, how);
            }
        }
    });
}

/// The running patch wrote code from `start` of method `handle`, in an edit that moved the
/// method's instructions as `map` describes.
pub(super) fn wrote(handle: u32, map: &InstructionMap, how: Moved, start: usize) {
    moved(handle, map, how);
    changed(handle, start);
}

/// The running patch removed instructions `removed` from method `handle`. Points on them go, the
/// rest follow `map`, and a point marks where the method goes on, unless they ended it.
pub(super) fn removed(handle: u32, removed: Range<usize>, map: &InstructionMap) {
    with_trace(|trace| {
        if trace.points.contains_key(&handle) {
            trace
                .points(handle)
                .retain(|point| !removed.contains(&point.index));
        }
    });
    moved(handle, map, Moved::Rewrite);
    let count = map.starts().len() - 1;
    if removed.end < count {
        changed(handle, map.starts()[removed.start]);
    }
}

/// The running patch gave method `handle` a new body, its code starting at `start`.
pub(super) fn body_replaced(handle: u32, start: usize) {
    code_removed(handle);
    changed(handle, start);
}

/// Method `handle` lost its code, and its points with it.
pub(super) fn code_removed(handle: u32) {
    with_trace(|trace| {
        if trace.points.contains_key(&handle) {
            trace.points(handle).clear();
        }
    });
}

pub(super) fn open_savepoint(savepoint: Savepoint) {
    with_trace(|trace| trace.journal.open(savepoint));
}

pub(super) fn release_savepoint(savepoint: Savepoint) {
    with_trace(|trace| trace.journal.release(savepoint));
}

pub(super) fn rollback_to(savepoint: Savepoint) {
    with_trace(|trace| {
        for (handle, points) in trace.journal.rollback(savepoint) {
            trace.points.insert(handle, points);
        }
    });
}

/// Adds the log calls of every point in a method the app still has.
pub(crate) fn insert(ctx: &mut PatchContext<'_>) -> Result<()> {
    let Some(trace) = TRACE.with(|trace| trace.borrow_mut().take()) else {
        return Ok(());
    };
    let mut methods: Vec<_> = trace
        .points
        .into_iter()
        .filter(|(_, points)| !points.is_empty())
        .collect();
    methods.sort_unstable_by_key(|&(handle, _)| handle);
    for (handle, points) in methods {
        if let Some(method) = with_handles(|handles| handles.get_method(handle)) {
            ctx.trace(method, points)?;
        }
    }
    Ok(())
}
