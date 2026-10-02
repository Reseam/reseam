// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::logged;
use crate::kotlin::handles::checked;
use crate::kotlin::handles::{code_mut, log_mutation, method_location, with_ctx, with_method_mut};
use crate::kotlin::link::link_method;
use crate::kotlin::types::MethodRef;
use boltffi::export;
use reseam_apk::reseam_dex::{Instruction as DexInsn, MethodIdx};

#[export]
pub fn replace_strings(m: u32, old: String, new: String, all: bool) -> u32 {
    with_method_mut(m, |dex, loc| {
        let Some(old) = dex.find_string_idx(&old) else {
            return Ok(None);
        };
        let new = dex.intern_string(&new);
        let Some(code) = logged("access method", code_mut(dex, loc))? else {
            return Ok(None);
        };
        logged(
            "replace strings",
            code.edit_instructions(|instructions| {
                let mut count = 0;
                for insn in instructions {
                    if let DexInsn::ConstString { string, .. }
                    | DexInsn::ConstStringJumbo { string, .. } = insn
                        && *string == old
                    {
                        *string = new;
                        count += 1;
                        if !all {
                            break;
                        }
                    }
                }
                Ok(count)
            }),
        )
        .map(Some)
    })
    .unwrap_or(0)
}

#[export]
pub fn replace_literals(m: u32, old: i64, new: i64, all: bool) -> u32 {
    let Some(location) = method_location(m) else {
        return 0;
    };
    crate::kotlin::handles::method_changed(m);
    with_ctx(|ctx| {
        let (count, first_error, skipped) = {
            let Some(dex) = checked(ctx.class_dex_mut(location.dex_idx, location.class_idx)) else {
                return 0;
            };
            let Some(code) = checked(code_mut(dex, location)) else {
                return 0;
            };
            let mut first_error = None;
            let mut skipped = 0;
            let count = code
                .edit_instructions(|instructions| {
                    let mut count = 0;
                    for insn in instructions
                        .iter_mut()
                        .filter(|insn| insn.literal() == Some(old))
                    {
                        match set_literal(insn, new) {
                            Ok(()) => count += 1,
                            Err(message) => {
                                first_error.get_or_insert_with(|| message.to_owned());
                                skipped += 1;
                            }
                        }
                        if !all {
                            break;
                        }
                    }
                    Ok(count)
                })
                .unwrap_or_else(|error| {
                    first_error = Some(error.to_string());
                    0
                });
            (count, first_error, skipped)
        };
        if let Some(message) = first_error {
            let suffix = if skipped == 1 {
                String::new()
            } else {
                format!(" ({skipped} instructions skipped)")
            };
            log_mutation::<()>(
                ctx,
                Err(format!(
                    "replace_literal skipped an instruction: old={old}, new={new}: {message}{suffix}"
                )),
            );
        }
        count
    })
}

#[export]
pub fn replace_method_call(
    m: u32,
    index: u32,
    new_class: String,
    new_name: String,
    new_proto: String,
) -> bool {
    link_method(&MethodRef {
        defining_class: new_class.clone(),
        name: new_name.clone(),
        proto: new_proto.clone(),
    });
    with_method_mut(m, |dex, loc| {
        let target = logged(
            "intern method",
            dex.intern_method(&new_class, &new_name, &new_proto),
        )?;
        let Some(code) = logged("access method", code_mut(dex, loc))? else {
            return Ok(None);
        };
        let Some(mut insn) = code.instructions().get(index as usize).cloned() else {
            return Ok(None);
        };
        set_method_ref(&mut insn, target)
            .map_err(|message| format!("replace_method_call failed at {index}: {message}"))?;
        logged(
            "replace method call",
            code.edit_instructions(|instructions| {
                instructions[index as usize] = insn;
                Ok(())
            }),
        )
        .map(Some)
    })
    .is_some()
}

fn set_literal(insn: &mut DexInsn, value: i64) -> Result<(), &'static str> {
    fn fit<T: TryFrom<i64>>(value: i64, what: &'static str) -> Result<T, &'static str> {
        T::try_from(value).map_err(|_| what)
    }
    match insn {
        DexInsn::Const4 { value: v, .. } => *v = fit(value, "literal does not fit const/4")?,
        DexInsn::Const16 { value: v, .. } => *v = fit(value, "literal does not fit const/16")?,
        DexInsn::Const { value: v, .. } => *v = fit(value, "literal does not fit const")?,
        DexInsn::ConstHigh16 { value: v, .. } => {
            if i64::from(value as i32) != value || value & 0xffff != 0 {
                return Err("literal does not fit const/high16");
            }
            *v = (value >> 16) as i16;
        }
        DexInsn::ConstWide16 { value: v, .. } => {
            *v = fit(value, "literal does not fit const-wide/16")?;
        }
        DexInsn::ConstWide32 { value: v, .. } => {
            *v = fit(value, "literal does not fit const-wide/32")?;
        }
        DexInsn::ConstWide { value: v, .. } => *v = value,
        DexInsn::ConstWideHigh16 { value: v, .. } => {
            if value & 0x0000_ffff_ffff_ffff != 0 {
                return Err("literal does not fit const-wide/high16");
            }
            *v = (value >> 48) as i16;
        }
        DexInsn::AddIntLit16 { literal, .. }
        | DexInsn::RsubIntLit16 { literal, .. }
        | DexInsn::MulIntLit16 { literal, .. }
        | DexInsn::DivIntLit16 { literal, .. }
        | DexInsn::RemIntLit16 { literal, .. }
        | DexInsn::AndIntLit16 { literal, .. }
        | DexInsn::OrIntLit16 { literal, .. }
        | DexInsn::XorIntLit16 { literal, .. } => {
            *literal = fit(value, "literal does not fit lit16 opcode")?;
        }
        DexInsn::AddIntLit8 { literal, .. }
        | DexInsn::RsubIntLit8 { literal, .. }
        | DexInsn::MulIntLit8 { literal, .. }
        | DexInsn::DivIntLit8 { literal, .. }
        | DexInsn::RemIntLit8 { literal, .. }
        | DexInsn::AndIntLit8 { literal, .. }
        | DexInsn::OrIntLit8 { literal, .. }
        | DexInsn::XorIntLit8 { literal, .. }
        | DexInsn::ShlIntLit8 { literal, .. }
        | DexInsn::ShrIntLit8 { literal, .. }
        | DexInsn::UshrIntLit8 { literal, .. } => {
            *literal = fit(value, "literal does not fit lit8 opcode")?;
        }
        _ => return Err("instruction does not carry a writable literal"),
    }
    Ok(())
}

fn set_method_ref(insn: &mut DexInsn, target: MethodIdx) -> Result<(), &'static str> {
    match insn {
        DexInsn::InvokeVirtual { method, .. }
        | DexInsn::InvokeSuper { method, .. }
        | DexInsn::InvokeDirect { method, .. }
        | DexInsn::InvokeStatic { method, .. }
        | DexInsn::InvokeInterface { method, .. }
        | DexInsn::InvokeVirtualRange { method, .. }
        | DexInsn::InvokeSuperRange { method, .. }
        | DexInsn::InvokeDirectRange { method, .. }
        | DexInsn::InvokeStaticRange { method, .. }
        | DexInsn::InvokeInterfaceRange { method, .. } => {
            *method = target;
            Ok(())
        }
        _ => Err("target instruction is not an invoke"),
    }
}
