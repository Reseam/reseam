// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cell::RefCell;

use reseam_dex::{CodeItem, DexFile, EncodedMethod, Instruction};

use super::table::HandleTable;
use crate::bridge::changes::{changed, method_changed};
use crate::bridge::phase::with_ctx;
use crate::bridge::run_state::{checked, record_failure};
use crate::context::{ClassLocation, MethodLocation};

thread_local! {
    static HANDLES: RefCell<HandleTable> = RefCell::new(HandleTable::default());
}

pub(crate) fn reset() {
    HANDLES.with(|handles| *handles.borrow_mut() = HandleTable::default());
}

pub(crate) fn with_handles<R>(f: impl FnOnce(&mut HandleTable) -> R) -> R {
    HANDLES.with(|handles| f(&mut handles.borrow_mut()))
}

pub(crate) fn alloc_method(location: MethodLocation) -> u32 {
    with_handles(|handles| handles.alloc_method(location))
}

pub(crate) fn alloc_methods(locations: impl IntoIterator<Item = MethodLocation>) -> Vec<u32> {
    with_handles(|handles| {
        locations
            .into_iter()
            .map(|location| handles.alloc_method(location))
            .collect()
    })
}

pub(crate) fn alloc_class(location: ClassLocation) -> u32 {
    with_handles(|handles| handles.alloc_class(location))
}

pub(crate) fn method_location(handle: u32) -> Option<MethodLocation> {
    let location = with_handles(|handles| handles.get_method(handle));
    if location.is_none() {
        record_failure(format!("invalid or removed method handle {handle}"));
    }
    location
}

pub(crate) fn class_location(handle: u32) -> Option<ClassLocation> {
    let location = with_handles(|handles| handles.get_class(handle));
    if location.is_none() {
        record_failure(format!("invalid or removed class handle {handle}"));
    }
    location
}

pub(crate) fn forget_method(location: MethodLocation) {
    with_handles(|handles| handles.forget_method(location));
    changed();
}

pub(crate) fn relocate_method(old: MethodLocation, new: MethodLocation) {
    with_handles(|handles| handles.relocate_method(old, new));
    changed();
}

pub(crate) fn forget_class(location: ClassLocation) {
    with_handles(|handles| handles.forget_class(location));
    changed();
}

fn with_method<R>(handle: u32, f: impl FnOnce(&DexFile, &EncodedMethod) -> Option<R>) -> Option<R> {
    let location = method_location(handle)?;
    with_ctx(|ctx| {
        let (dex, method) = checked(ctx.read_method(location))?;
        f(dex, method)
    })
}

pub(crate) fn with_code<R>(
    handle: u32,
    f: impl FnOnce(&DexFile, &CodeItem) -> Option<R>,
) -> Option<R> {
    with_method(handle, |dex, method| f(dex, method.code.as_ref()?))
}

pub(crate) fn with_instruction<R>(
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

pub(crate) fn with_method_mut<R>(
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
        checked(result)
    })
}

pub(crate) fn with_class_mut<R>(
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
        checked(result)
    })
}

pub(crate) fn method_ref(dex: &DexFile, m: MethodLocation) -> Option<&EncodedMethod> {
    let data = dex.classes().resident(m.class_idx)?.class_data.as_ref()?;
    data.methods(m.kind).get(m.method_idx)
}
