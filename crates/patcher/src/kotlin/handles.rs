// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::PathBuf;
#[cfg(feature = "kotlin")]
use std::sync::Arc;

pub(super) use super::handle_table::HandleSpace;
use super::handle_table::HandleTable;
#[cfg(feature = "kotlin")]
use jni::objects::{Global, JObject};
use reseam_apk::reseam_dex::{CodeItem, DexFile, EncodedMethod, Instruction};
use rustc_hash::FxHashMap;

use super::types::PoolOrigin;
use super::xml;
use reseam_apk::reseam_dex::MethodKind;

use crate::error::{PatcherError, Result as PatcherResult};

use crate::context::{ClassLocation, MethodLocation, PatchContext};
pub(super) use crate::context::{code_mut, method_mut};

thread_local! {
    #[cfg(feature = "kotlin")]
    static KOTLIN_RUN: RefCell<Option<Arc<Global<JObject<'static>>>>> = const { RefCell::new(None) };
    static RUN_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static CTX_PTR: Cell<*mut ()> = const { Cell::new(std::ptr::null_mut()) };
    static HANDLES: RefCell<HandleTable> = RefCell::new(HandleTable::default());
    static FAILURE: RefCell<Option<String>> = const { RefCell::new(None) };
    static REVISION: Cell<u64> = const { Cell::new(0) };
    static CHANGES: RefCell<VecDeque<MethodChange>> = const { RefCell::new(VecDeque::new()) };
    static STRUCTURAL_REVISION: Cell<u64> = const { Cell::new(0) };
    static POOL_COPIES: RefCell<FxHashMap<PoolCopy, u32>> = RefCell::new(FxHashMap::default());
    static BUNDLE_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PoolKind {
    Handle,
    CallSite,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PoolCopy {
    origin: PoolOrigin,
    destination: u32,
    kind: PoolKind,
}

pub(super) fn copied_pool(origin: PoolOrigin, destination: u32, kind: PoolKind) -> Option<u32> {
    POOL_COPIES.with(|copies| {
        copies
            .borrow()
            .get(&PoolCopy {
                origin,
                destination,
                kind,
            })
            .copied()
    })
}

pub(super) fn remember_pool(origin: PoolOrigin, destination: u32, kind: PoolKind, index: u32) {
    POOL_COPIES.with(|copies| {
        copies.borrow_mut().insert(
            PoolCopy {
                origin,
                destination,
                kind,
            },
            index,
        )
    });
}

pub(super) struct ContextGuard<'ctx, 'apk> {
    _context: std::marker::PhantomData<&'ctx mut PatchContext<'apk>>,
}

pub(crate) struct RunGuard;

impl RunGuard {
    pub fn enter() -> PatcherResult<Self> {
        if RUN_ACTIVE.with(|active| active.replace(true)) {
            return Err(PatcherError::Bridge(
                "nested patch run on the same thread".into(),
            ));
        }
        reset();
        Ok(Self)
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        reset();
        RUN_ACTIVE.with(|active| active.set(false));
    }
}

#[cfg(feature = "kotlin")]
pub(super) fn kotlin_run(
    create: impl FnOnce() -> PatcherResult<Global<JObject<'static>>>,
) -> PatcherResult<Arc<Global<JObject<'static>>>> {
    KOTLIN_RUN.with(|run| {
        let mut run = run.borrow_mut();
        if let Some(run) = run.as_ref() {
            return Ok(run.clone());
        }
        let value = Arc::new(create()?);
        *run = Some(value.clone());
        Ok(value)
    })
}

fn reset() {
    super::hermes::reset();
    #[cfg(feature = "kotlin")]
    KOTLIN_RUN.with(|run| *run.borrow_mut() = None);
    POOL_COPIES.with(|copies| copies.borrow_mut().clear());
    HANDLES.with(|handles| *handles.borrow_mut() = HandleTable::default());
    FAILURE.with(|failure| *failure.borrow_mut() = None);
    REVISION.with(|revision| revision.set(0));
    STRUCTURAL_REVISION.with(|revision| revision.set(0));
    CHANGES.with(|changes| changes.borrow_mut().clear());
    xml::reset();
}

pub(super) fn record_failure(error: impl std::fmt::Display) {
    FAILURE.with(|failure| {
        failure
            .borrow_mut()
            .get_or_insert_with(|| error.to_string());
    });
}

pub(super) fn checked<T: Default>(result: Result<T, impl std::fmt::Display>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            record_failure(error);
            T::default()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn reseam_patch_call_failed() -> i32 {
    i32::from(check_call().is_err())
}

#[boltffi::export]
pub fn check_call() -> Result<(), String> {
    if !context_is_active() {
        return Err("patch callbacks require an active context on this thread".into());
    }
    FAILURE.with(|failure| {
        failure
            .borrow()
            .as_ref()
            .map_or(Ok(()), |reason| Err(reason.clone()))
    })
}

#[boltffi::export]
pub fn check_invocation() -> Result<(), String> {
    FAILURE
        .with(|failure| {
            failure
                .borrow()
                .as_ref()
                .map_or(Ok(()), |reason| Err(reason.clone()))
        })
        .and_then(|()| xml::finish())
}

pub(super) fn changed() {
    REVISION.with(|revision| revision.set(revision.get().wrapping_add(1)));
    STRUCTURAL_REVISION.with(|revision| revision.set(mutation_revision()));
    CHANGES.with(|changes| changes.borrow_mut().clear());
}

struct MethodChange {
    revision: u64,
    handle: u32,
}

pub(super) fn method_changed(handle: u32) {
    REVISION.with(|revision| revision.set(revision.get().wrapping_add(1)));
    CHANGES.with(|changes| {
        let mut changes = changes.borrow_mut();
        if changes.len() == 1024 {
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

impl<'ctx, 'apk> ContextGuard<'ctx, 'apk> {
    pub fn enter(ctx: &'ctx mut PatchContext<'apk>, bundle_dir: PathBuf) -> PatcherResult<Self> {
        CTX_PTR.with(|cell| {
            if !cell.get().is_null() {
                return Err(PatcherError::Bridge("nested patch context".into()));
            }
            cell.set(std::ptr::from_mut(ctx).cast());
            Ok(())
        })?;
        BUNDLE_DIR.with(|dir| *dir.borrow_mut() = Some(bundle_dir));
        FAILURE.with(|failure| *failure.borrow_mut() = None);
        Ok(Self {
            _context: std::marker::PhantomData,
        })
    }

    pub fn finish(self) -> PatcherResult<()> {
        if let Err(error) = xml::finish() {
            record_failure(error);
        }
        let outcome = FAILURE.with(|failure| {
            failure
                .borrow_mut()
                .take()
                .map_or(Ok(()), |reason| Err(PatcherError::Bridge(reason)))
        });
        drop(self);
        outcome
    }
}

impl Drop for ContextGuard<'_, '_> {
    fn drop(&mut self) {
        CTX_PTR.with(|cell| cell.set(std::ptr::null_mut()));
        BUNDLE_DIR.with(|dir| *dir.borrow_mut() = None);
        xml::reset();
    }
}

pub(super) fn context_is_active() -> bool {
    CTX_PTR.with(|cell| !cell.get().is_null())
}

pub(super) fn with_ctx<R: Default>(f: impl FnOnce(&mut PatchContext<'_>) -> R) -> R {
    checked(try_with_ctx(f))
}

pub(super) fn with_ctx_result<R>(
    f: impl FnOnce(&mut PatchContext<'_>) -> Result<R, String>,
) -> Result<R, String> {
    try_with_ctx(f).map_err(|error| error.to_string())?
}

pub(super) fn try_with_ctx<R>(f: impl FnOnce(&mut PatchContext<'_>) -> R) -> PatcherResult<R> {
    CTX_PTR.with(|cell| {
        let ptr = cell.get();
        if ptr.is_null() {
            return Err(PatcherError::Bridge(
                "patch context is not active on this thread".into(),
            ));
        }
        // SAFETY: the pointer was set from a live `&mut PatchContext` by
        // `ContextGuard::enter` on this thread and is cleared before that
        // borrow ends; exported functions run on the same thread, one at a time.
        Ok(f(unsafe { &mut *ptr.cast::<PatchContext<'_>>() }))
    })
}

pub(super) fn bundle_path(relative: &str) -> PathBuf {
    BUNDLE_DIR.with(|dir| match dir.borrow().as_ref() {
        Some(dir) => dir.join(relative),
        None => PathBuf::from(relative),
    })
}

pub(super) fn alloc_method(location: MethodLocation) -> u32 {
    HANDLES.with(|h| h.borrow_mut().alloc_method(location))
}

pub(super) fn alloc_methods(locations: impl IntoIterator<Item = MethodLocation>) -> Vec<u32> {
    HANDLES.with(|h| {
        let mut h = h.borrow_mut();
        locations.into_iter().map(|l| h.alloc_method(l)).collect()
    })
}

pub(super) fn method_location(handle: u32) -> Option<MethodLocation> {
    let location = HANDLES.with(|h| h.borrow().get_method(handle));
    if location.is_none() {
        record_failure(format!("invalid or removed method handle {handle}"));
    }
    location
}

pub(super) fn forget_method(location: MethodLocation) {
    HANDLES.with(|h| h.borrow_mut().forget_method(location));
    changed();
}

pub(super) fn relocate_method(old: MethodLocation, new: MethodLocation) {
    HANDLES.with(|handles| handles.borrow_mut().relocate_method(old, new));
    changed();
}

pub(super) fn forget_class(location: ClassLocation) {
    HANDLES.with(|handles| handles.borrow_mut().forget_class(location));
    changed();
}

pub(super) fn alloc_class(location: ClassLocation) -> u32 {
    HANDLES.with(|h| h.borrow_mut().alloc_class(location))
}

pub(super) fn class_location(handle: u32) -> Option<ClassLocation> {
    let location = HANDLES.with(|h| h.borrow().get_class(handle));
    if location.is_none() {
        record_failure(format!("invalid or removed class handle {handle}"));
    }
    location
}

pub(super) fn with_method<R>(
    handle: u32,
    f: impl FnOnce(&DexFile, &EncodedMethod) -> Option<R>,
) -> Option<R> {
    let location = method_location(handle)?;
    with_ctx(|ctx| {
        let (dex, method) = checked(ctx.read_method(location))?;
        f(dex, method)
    })
}

pub(super) fn with_code<R>(
    handle: u32,
    f: impl FnOnce(&DexFile, &CodeItem) -> Option<R>,
) -> Option<R> {
    with_method(handle, |dex, method| f(dex, method.code.as_ref()?))
}

pub(super) fn with_instruction<R>(
    handle: u32,
    index: u32,
    f: impl FnOnce(&DexFile, &Instruction) -> Option<R>,
) -> Option<R> {
    with_method(handle, |dex, method| {
        let instruction = method
            .code
            .as_ref()
            .and_then(|code| code.instructions().get(index as usize));
        let Some(instruction) = instruction else {
            record_failure(format!(
                "method {handle} has no instruction at index {index}"
            ));
            return None;
        };
        f(dex, instruction)
    })
}

pub(super) fn with_method_mut<R>(
    handle: u32,
    f: impl FnOnce(&mut DexFile, MethodLocation) -> Result<Option<R>, String>,
) -> Option<R> {
    let location = method_location(handle)?;
    method_changed(handle);
    with_ctx(|ctx| {
        let result = f(
            checked(ctx.class_dex_mut(location.dex_idx, location.class_idx))?,
            location,
        );
        log_mutation(ctx, result)
    })
}

pub(super) fn with_class_mut<R>(
    handle: u32,
    f: impl FnOnce(&mut DexFile, ClassLocation) -> Result<Option<R>, String>,
) -> Option<R> {
    let location = class_location(handle)?;
    changed();
    with_ctx(|ctx| {
        let result = f(
            checked(ctx.class_dex_mut(location.dex_idx, location.class_idx))?,
            location,
        );
        log_mutation(ctx, result)
    })
}

pub(super) fn log_mutation<R>(
    ctx: &mut PatchContext<'_>,
    result: Result<Option<R>, String>,
) -> Option<R> {
    result.unwrap_or_else(|message| {
        record_failure(&message);
        ctx.log().warn(message);
        None
    })
}

pub(super) fn method_ref(dex: &DexFile, m: MethodLocation) -> Option<&EncodedMethod> {
    let data = dex.resident_class(m.class_idx)?.class_data.as_ref()?;
    let list = if m.kind == MethodKind::Virtual {
        &data.virtual_methods
    } else {
        &data.direct_methods
    };
    list.get(m.method_idx)
}
