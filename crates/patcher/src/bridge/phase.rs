// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The patch phase the exports act on: the context a patch runs against and
//! the bundle it came from, set for the length of one patch invocation.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
#[cfg(any(feature = "kotlin", feature = "browser"))]
use std::sync::Arc;

#[cfg(any(feature = "kotlin", feature = "browser"))]
use reseam_storage::ScratchDir;

use crate::bridge::apk::xml;
use crate::bridge::run_state::{checked, clear_failure, record_failure, take_failure};
#[cfg(any(feature = "kotlin", feature = "browser"))]
use crate::bundle::BundleInfo;
use crate::context::PatchContext;
use crate::error::{PatcherError, Result as PatcherResult};
use crate::log::PatchScope;
use crate::output::OutputContext;

thread_local! {
    static CTX_PTR: Cell<*mut ()> = const { Cell::new(std::ptr::null_mut()) };
    static OUTPUT_PTR: Cell<*mut ()> = const { Cell::new(std::ptr::null_mut()) };
    static BUNDLE: RefCell<Option<RunningBundle>> = const { RefCell::new(None) };
}

/// The bundle whose patch is running: who it is and where its files are.
pub(crate) struct RunningBundle {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) license: String,
    pub(crate) directory: PathBuf,
}

/// A loaded bundle's identity and extracted files, held by its patches.
#[cfg(any(feature = "kotlin", feature = "browser"))]
pub(crate) struct LoadedBundle {
    name: String,
    version: String,
    license: String,
    directory: Arc<ScratchDir>,
}

#[cfg(any(feature = "kotlin", feature = "browser"))]
impl LoadedBundle {
    pub(crate) fn new(info: &BundleInfo, directory: Arc<ScratchDir>) -> Self {
        Self {
            name: info.name.clone(),
            version: info.version.clone(),
            license: info.license.clone(),
            directory,
        }
    }

    pub(crate) fn running(&self) -> RunningBundle {
        RunningBundle {
            name: self.name.clone(),
            version: self.version.clone(),
            license: self.license.clone(),
            directory: self.directory.path().to_path_buf(),
        }
    }
}

#[cfg(test)]
impl RunningBundle {
    pub(crate) fn at(directory: PathBuf) -> Self {
        Self {
            name: "test".into(),
            version: String::new(),
            license: String::new(),
            directory,
        }
    }
}

/// Makes `ctx` the patch context of the exports a patch calls while it runs.
pub(super) struct ContextGuard<'ctx, 'apk> {
    _context: std::marker::PhantomData<&'ctx mut PatchContext<'apk>>,
}

impl<'ctx, 'apk> ContextGuard<'ctx, 'apk> {
    pub fn enter(ctx: &'ctx mut PatchContext<'apk>, bundle: RunningBundle) -> PatcherResult<Self> {
        enter(&CTX_PTR, std::ptr::from_mut(ctx).cast(), bundle)?;
        Ok(Self {
            _context: std::marker::PhantomData,
        })
    }

    pub fn finish(self) -> PatcherResult<()> {
        if let Err(error) = xml::finish() {
            record_failure(error);
        }
        let outcome = take_failure();
        drop(self);
        outcome
    }
}

impl Drop for ContextGuard<'_, '_> {
    fn drop(&mut self) {
        leave(&CTX_PTR);
        xml::reset();
    }
}

/// Makes `ctx` the output context of the exports a patch calls while it
/// rewrites output.
#[cfg(feature = "kotlin")]
pub(super) struct OutputGuard<'ctx, 'apk> {
    _context: std::marker::PhantomData<&'ctx mut OutputContext<'apk>>,
}

#[cfg(feature = "kotlin")]
impl<'ctx, 'apk> OutputGuard<'ctx, 'apk> {
    pub fn enter(ctx: &'ctx mut OutputContext<'apk>, bundle: RunningBundle) -> PatcherResult<Self> {
        enter(&OUTPUT_PTR, std::ptr::from_mut(ctx).cast(), bundle)?;
        Ok(Self {
            _context: std::marker::PhantomData,
        })
    }

    pub fn finish(self) -> PatcherResult<()> {
        let outcome = take_failure();
        drop(self);
        outcome
    }
}

#[cfg(feature = "kotlin")]
impl Drop for OutputGuard<'_, '_> {
    fn drop(&mut self) {
        leave(&OUTPUT_PTR);
    }
}

fn enter(
    slot: &'static std::thread::LocalKey<Cell<*mut ()>>,
    ctx: *mut (),
    bundle: RunningBundle,
) -> PatcherResult<()> {
    if is_active() {
        return Err(PatcherError::Bridge("nested patch context".into()));
    }
    slot.with(|cell| cell.set(ctx));
    BUNDLE.with(|running| *running.borrow_mut() = Some(bundle));
    clear_failure();
    Ok(())
}

fn leave(slot: &'static std::thread::LocalKey<Cell<*mut ()>>) {
    slot.with(|cell| cell.set(std::ptr::null_mut()));
    BUNDLE.with(|running| *running.borrow_mut() = None);
}

fn context_is_active() -> bool {
    CTX_PTR.with(|cell| !cell.get().is_null())
}

pub(super) fn is_active() -> bool {
    context_is_active() || OUTPUT_PTR.with(|cell| !cell.get().is_null())
}

/// Runs `f` with the log and options of the patch whose phase is running.
pub(super) fn with_patch<R: Default>(f: impl FnOnce(&mut PatchScope) -> R) -> R {
    if context_is_active() {
        return with_ctx(|ctx| f(ctx.scope()));
    }
    with_output(|ctx| f(ctx.scope()))
}

pub(super) fn with_output<R: Default>(f: impl FnOnce(&mut OutputContext<'_>) -> R) -> R {
    checked(OUTPUT_PTR.with(|cell| {
        let ptr = cell.get();
        if ptr.is_null() {
            return Err(PatcherError::Bridge(
                "this call is only available while a patch rewrites output".into(),
            ));
        }
        // SAFETY: the pointer was set from a live `&mut OutputContext` by
        // `OutputGuard::enter` on this thread and is cleared before that borrow
        // ends; exported functions run on the same thread, one at a time.
        Ok(f(unsafe { &mut *ptr.cast::<OutputContext<'_>>() }))
    }))
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
    BUNDLE.with(|running| match running.borrow().as_ref() {
        Some(bundle) => bundle.directory.join(relative),
        None => PathBuf::from(relative),
    })
}

pub(super) fn with_running_bundle<R>(f: impl FnOnce(&RunningBundle) -> R) -> Result<R, String> {
    BUNDLE.with(|running| {
        running
            .borrow()
            .as_ref()
            .map(f)
            .ok_or_else(|| "no patch is running on this thread".to_owned())
    })
}
