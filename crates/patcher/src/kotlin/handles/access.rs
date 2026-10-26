// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::{CodeItem, DexFile, EncodedMethod, Instruction, MethodKind};

use super::{
    changed, checked, class_location, method_changed, method_location, record_failure, with_ctx,
};
use crate::context::{ClassLocation, MethodLocation, PatchContext};

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
        log_mutation(ctx, result)
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
        log_mutation(ctx, result)
    })
}

pub(crate) fn log_mutation<R>(
    ctx: &mut PatchContext<'_>,
    result: Result<Option<R>, String>,
) -> Option<R> {
    result.unwrap_or_else(|message| {
        record_failure(&message);
        ctx.log().warn(message);
        None
    })
}

pub(crate) fn method_ref(dex: &DexFile, m: MethodLocation) -> Option<&EncodedMethod> {
    let data = dex.classes().resident(m.class_idx)?.class_data.as_ref()?;
    let list = if m.kind == MethodKind::Virtual {
        &data.virtual_methods
    } else {
        &data.direct_methods
    };
    list.get(m.method_idx)
}
