// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the bridge holds for one patch run on this thread: whether a run is
//! active, the first failure of the current call, and the Kotlin patch runtime.

use std::cell::{Cell, RefCell};
#[cfg(feature = "kotlin")]
use std::sync::Arc;

#[cfg(feature = "kotlin")]
use jni::objects::{Global, JObject};

use super::apk::xml;
use super::dex::pool_copies;
use super::{changes, handles, hermes, phase};
use crate::error::{PatcherError, Result as PatcherResult};

thread_local! {
    #[cfg(feature = "kotlin")]
    static KOTLIN_RUN: RefCell<Option<Arc<Global<JObject<'static>>>>> = const { RefCell::new(None) };
    static RUN_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static FAILURE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Holds the thread's run state from entry until drop; runs do not nest.
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

fn reset() {
    hermes::reset();
    #[cfg(feature = "kotlin")]
    KOTLIN_RUN.with(|run| *run.borrow_mut() = None);
    pool_copies::reset();
    handles::reset();
    changes::reset();
    clear_failure();
    xml::reset();
}

/// The run's Kotlin patch runtime, once a patch has created it.
#[cfg(feature = "kotlin")]
pub(super) fn existing_kotlin_run() -> Option<Arc<Global<JObject<'static>>>> {
    KOTLIN_RUN.with(|run| run.borrow().clone())
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

/// Records why the current call failed; the first reason wins.
pub(super) fn record_failure(error: impl std::fmt::Display) {
    FAILURE.with(|failure| {
        failure
            .borrow_mut()
            .get_or_insert_with(|| error.to_string());
    });
}

/// The value of `result`, or the default after recording its error.
pub(super) fn checked<T: Default>(result: Result<T, impl std::fmt::Display>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            record_failure(error);
            T::default()
        }
    }
}

pub(super) fn clear_failure() {
    FAILURE.with(|failure| *failure.borrow_mut() = None);
}

pub(super) fn take_failure() -> PatcherResult<()> {
    FAILURE.with(|failure| {
        failure
            .borrow_mut()
            .take()
            .map_or(Ok(()), |reason| Err(PatcherError::Bridge(reason)))
    })
}

fn failure() -> Result<(), String> {
    FAILURE.with(|failure| {
        failure
            .borrow()
            .as_ref()
            .map_or(Ok(()), |reason| Err(reason.clone()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn reseam_patch_call_failed() -> i32 {
    i32::from(check_call().is_err())
}

#[boltffi::export]
pub fn check_call() -> Result<(), String> {
    if !phase::is_active() {
        return Err("patch callbacks require a running patch phase on this thread".into());
    }
    failure()
}

#[boltffi::export]
pub fn check_invocation() -> Result<(), String> {
    failure().and_then(|()| xml::finish())
}
