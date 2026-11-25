// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cell::RefCell;
use std::thread::LocalKey;

use boltffi::export;
use reseam_elf::{Branch, Function, Pattern};
use reseam_model::Abi;

use crate::bridge::handles::OwnedHandles;
use crate::bridge::phase::with_ctx_result;
use crate::context::{LibraryKey, NativeQuery};

type Handles<T> = RefCell<OwnedHandles<Vec<(LibraryKey, T)>>>;

thread_local! {
    /// Each resolved function target: the matched function in each copy of the library.
    static FUNCTIONS: Handles<Function> = RefCell::default();
    /// Each resolved branch target: the matched branch in each copy of the library.
    static BRANCHES: Handles<Branch> = RefCell::default();
}

pub(super) fn reset() {
    FUNCTIONS.with(|functions| *functions.borrow_mut() = OwnedHandles::default());
    BRANCHES.with(|branches| *branches.borrow_mut() = OwnedHandles::default());
}

/// An ABI whose native code can be edited.
#[boltffi::data]
#[derive(Debug, Clone, Copy)]
pub enum NativeLibraryAbi {
    Arm64V8a,
    ArmeabiV7a,
}

impl From<NativeLibraryAbi> for Abi {
    fn from(abi: NativeLibraryAbi) -> Self {
        match abi {
            NativeLibraryAbi::Arm64V8a => Abi::Arm64V8a,
            NativeLibraryAbi::ArmeabiV7a => Abi::ArmeabiV7a,
        }
    }
}

/// A byte pattern in the code of the library's copy for `abi`, such as `"1f 20 ?? d5"`.
#[boltffi::data]
#[derive(Debug, Clone)]
pub struct NativeBytes {
    pub abi: NativeLibraryAbi,
    pub pattern: String,
}

fn patterns(bytes: Vec<NativeBytes>) -> Result<Vec<(Abi, Pattern)>, String> {
    bytes
        .into_iter()
        .map(|bytes| {
            let pattern = bytes
                .pattern
                .parse::<Pattern>()
                .map_err(|e| e.to_string())?;
            Ok((bytes.abi.into(), pattern))
        })
        .collect()
}

fn insert<T>(
    handles: &'static LocalKey<Handles<T>>,
    found: Vec<(LibraryKey, T)>,
) -> Result<u32, String> {
    handles.with(|handles| handles.borrow_mut().insert(found))
}

/// What `handle` resolved to in each copy of the library.
fn resolved<T: Clone>(
    handles: &'static LocalKey<Handles<T>>,
    handle: u32,
) -> Result<Vec<(LibraryKey, T)>, String> {
    handles.with(|handles| {
        handles
            .borrow()
            .get(handle)
            .cloned()
            .ok_or_else(|| "stale native target handle".to_owned())
    })
}

#[export]
pub fn native_find_function(
    library: String,
    abi: Option<NativeLibraryAbi>,
    symbol: Option<String>,
    strings: Vec<String>,
    bytes: Vec<NativeBytes>,
    nth: Option<u32>,
) -> Result<u32, String> {
    let patterns = patterns(bytes)?;
    with_ctx_result(|ctx| {
        let strings: Vec<&str> = strings.iter().map(String::as_str).collect();
        let found = ctx
            .native_find(&NativeQuery {
                library: &library,
                abi: abi.map(Abi::from),
                symbol: symbol.as_deref(),
                strings: &strings,
                patterns: &patterns,
                nth: nth.map(|index| index as usize),
            })
            .map_err(|e| e.to_string())?;
        insert(&FUNCTIONS, found)
    })
}

#[export]
pub fn native_find_branch(function: u32, bytes: Vec<NativeBytes>) -> Result<u32, String> {
    let patterns = patterns(bytes)?;
    let functions = resolved(&FUNCTIONS, function)?;
    with_ctx_result(|ctx| {
        let found = ctx
            .native_find_branch(&functions, &patterns)
            .map_err(|e| e.to_string())?;
        insert(&BRANCHES, found)
    })
}

#[export]
pub fn native_always_return(function: u32, value: i32) -> Result<(), String> {
    let functions = resolved(&FUNCTIONS, function)?;
    with_ctx_result(|ctx| {
        ctx.native_always_return(&functions, value)
            .map_err(|e| e.to_string())
    })
}

#[export]
pub fn native_replace_bytes(function: u32, offset: u64, bytes: Vec<u8>) -> Result<(), String> {
    let functions = resolved(&FUNCTIONS, function)?;
    with_ctx_result(|ctx| {
        ctx.native_replace_bytes(&functions, offset, &bytes)
            .map_err(|e| e.to_string())
    })
}

#[export]
pub fn native_always_take(branch: u32) -> Result<(), String> {
    let branches = resolved(&BRANCHES, branch)?;
    with_ctx_result(|ctx| ctx.native_always_take(&branches).map_err(|e| e.to_string()))
}

#[export]
pub fn native_never_take(branch: u32) -> Result<(), String> {
    let branches = resolved(&BRANCHES, branch)?;
    with_ctx_result(|ctx| ctx.native_never_take(&branches).map_err(|e| e.to_string()))
}
