// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Instruction-level mutation of one method.

use boltffi::export;
use reseam_apk::reseam_dex::{
    find_contiguous_free_registers, CodeItem, Instruction as DexInsn, MethodIdx, RegList,
};
use tracing::warn;

use crate::kotlin::convert::kotlin_to_dex;
use crate::kotlin::handles::{code_mut, method_mut, with_method_mut};
use crate::kotlin::types::Instruction;

/// Converts instructions with the DEX, then edits the method's code.
fn edit_code<R>(
    m: u32,
    insns: &[Instruction],
    f: impl FnOnce(&mut CodeItem, Vec<DexInsn>) -> Option<R>,
) -> Option<R> {
    with_method_mut(m, |dex, loc| {
        let insns: Vec<DexInsn> = insns.iter().map(|insn| kotlin_to_dex(insn, dex)).collect();
        f(code_mut(dex, loc)?, insns)
    })
}

fn logged<T>(what: &str, result: reseam_apk::reseam_dex::Result<T>) -> Option<T> {
    result.map_err(|error| warn!(%error, "{what} failed")).ok()
}

#[export]
pub fn set_instructions(m: u32, insns: Vec<Instruction>) {
    edit_code(m, &insns, |code, insns| {
        code.set_instructions(insns);
        Some(())
    });
}

/// Replaces the whole body, dropping debug info the new code cannot match.
#[export]
pub fn replace_body(m: u32, registers_size: u16, outs_size: u16, insns: Vec<Instruction>) {
    edit_code(m, &insns, |code, insns| {
        code.set_instructions(insns);
        code.registers_size = registers_size;
        code.outs_size = outs_size;
        code.debug_info = None;
        Some(())
    });
}

#[export]
pub fn insert_instructions(m: u32, index: u32, insns: Vec<Instruction>) {
    edit_code(m, &insns, |code, insns| {
        logged(
            "insert_instructions",
            code.insert_instructions(index as usize, &insns),
        )
    });
}

#[export]
pub fn replace_instruction(m: u32, index: u32, insn: Instruction) {
    edit_code(m, std::slice::from_ref(&insn), |code, mut insns| {
        logged(
            "replace_instruction",
            code.replace_instruction(index as usize, insns.remove(0)),
        )
    });
}

#[export]
pub fn remove_instructions(m: u32, index: u32, count: u32) {
    with_method_mut(m, |dex, loc| {
        let code = code_mut(dex, loc)?;
        for _ in 0..count {
            if (index as usize) >= code.instructions.len() {
                break;
            }
            logged(
                "remove_instruction",
                code.remove_instruction(index as usize),
            )?;
        }
        Some(())
    });
}

#[export]
pub fn return_early(m: u32) {
    with_method_mut(m, |dex, loc| {
        method_mut(dex, loc)?.return_early();
        Some(())
    });
}

#[export]
pub fn return_early_int(m: u32, value: i32) {
    with_method_mut(m, |dex, loc| {
        method_mut(dex, loc)?.return_early_int(value);
        Some(())
    });
}

#[export]
pub fn return_early_object_null(m: u32) {
    set_body(
        m,
        vec![
            DexInsn::Const4 { dest: 0, value: 0 },
            DexInsn::ReturnObject { src: 0 },
        ],
    );
}

#[export]
pub fn return_early_wide(m: u32, value: i64) {
    set_body(
        m,
        vec![
            DexInsn::ConstWide { dest: 0, value },
            DexInsn::ReturnWide { src: 0 },
        ],
    );
}

fn set_body(m: u32, insns: Vec<DexInsn>) {
    with_method_mut(m, |dex, loc| {
        code_mut(dex, loc)?.set_instructions(insns);
        Some(())
    });
}

/// Rewrites string constants equal to `old`; `all` decides whether to stop
/// after the first. Returns how many changed.
#[export]
pub fn replace_strings(m: u32, old: String, new: String, all: bool) -> u32 {
    with_method_mut(m, |dex, loc| {
        let old = dex.find_string_idx(&old)?;
        let new = dex.intern_string(&new);
        let code = code_mut(dex, loc)?;
        let mut count = 0;
        for insn in &mut code.instructions {
            if let DexInsn::ConstString { string, .. } | DexInsn::ConstStringJumbo { string, .. } =
                insn
            {
                if *string == old {
                    *string = new;
                    count += 1;
                    if !all {
                        break;
                    }
                }
            }
        }
        Some(count)
    })
    .unwrap_or(0)
}

/// Rewrites literals equal to `old` where the instruction can encode `new`;
/// `all` decides whether to stop after the first. Returns how many changed.
#[export]
pub fn replace_literals(m: u32, old: i64, new: i64, all: bool) -> u32 {
    with_method_mut(m, |dex, loc| {
        let code = code_mut(dex, loc)?;
        let mut count = 0;
        for insn in code
            .instructions
            .iter_mut()
            .filter(|insn| insn.literal() == Some(old))
        {
            match set_literal(insn, new) {
                Ok(()) => count += 1,
                Err(message) => {
                    warn!(old, new, message, "replace_literal skipped an instruction");
                    if !all {
                        return Some(0);
                    }
                }
            }
            if !all {
                break;
            }
        }
        Some(count)
    })
    .unwrap_or(0)
}

#[export]
pub fn replace_method_call(
    m: u32,
    index: u32,
    new_class: String,
    new_name: String,
    new_proto: String,
) -> bool {
    with_method_mut(m, |dex, loc| {
        let target = dex.intern_method(&new_class, &new_name, &new_proto).ok()?;
        let insn = code_mut(dex, loc)?.instructions.get_mut(index as usize)?;
        set_method_ref(insn, target)
            .map_err(|message| warn!(index, message, "replace_method_call failed"))
            .ok()
    })
    .is_some()
}

#[export]
pub fn insert_invoke_static(
    m: u32,
    index: u32,
    class_name: String,
    name: String,
    proto: String,
    registers: Vec<u16>,
) -> bool {
    insert_invoke(m, index, &class_name, &name, &proto, &registers, None)
}

/// Inserts a static call followed by a `move-result` into `result_register`.
#[export]
#[allow(clippy::too_many_arguments)]
pub fn insert_invoke_static_with_move_result(
    m: u32,
    index: u32,
    class_name: String,
    name: String,
    proto: String,
    registers: Vec<u16>,
    result_register: u16,
    is_object: bool,
) -> bool {
    insert_invoke(
        m,
        index,
        &class_name,
        &name,
        &proto,
        &registers,
        Some((result_register, is_object)),
    )
}

fn insert_invoke(
    m: u32,
    index: u32,
    class: &str,
    name: &str,
    proto: &str,
    registers: &[u16],
    move_result: Option<(u16, bool)>,
) -> bool {
    with_method_mut(m, |dex, loc| {
        let method = dex.intern_method(class, name, proto).ok()?;
        let code = code_mut(dex, loc)?;
        let mut lowered = lower_static_invoke(code, index as usize, method, proto, registers)?;
        if let Some((dest, is_object)) = move_result {
            let dest = u8::try_from(dest)
                .map_err(|_| warn!(dest, "move-result destination exceeds v255"))
                .ok()?;
            lowered.push(if is_object {
                DexInsn::MoveResultObject { dest }
            } else {
                DexInsn::MoveResult { dest }
            });
        }
        logged(
            "insert_invoke_static",
            code.insert_instructions(index as usize, &lowered),
        )
    })
    .is_some()
}

#[derive(Clone, Copy)]
enum MoveKind {
    Narrow,
    Wide,
    Object,
}

/// `invoke-static` over arbitrary registers: direct when they fit the
/// 4-bit form, `/range` when consecutive, otherwise moved into a scratch
/// range first.
fn lower_static_invoke(
    code: &CodeItem,
    index: usize,
    method: MethodIdx,
    proto: &str,
    registers: &[u16],
) -> Option<Vec<DexInsn>> {
    let count = u8::try_from(registers.len())
        .map_err(|_| {
            warn!(
                register_count = registers.len(),
                "invoke-static register count exceeds range encoding"
            )
        })
        .ok()?;
    if registers.len() <= 5 && registers.iter().all(|&reg| reg <= 15) {
        let args: RegList = registers.iter().map(|&r| r as u8).collect();
        return Some(vec![DexInsn::InvokeStatic { method, args }]);
    }
    if registers
        .windows(2)
        .all(|pair| pair[1] == pair[0].saturating_add(1))
    {
        let first_reg = registers.first().copied().unwrap_or(0);
        return Some(vec![DexInsn::InvokeStaticRange {
            method,
            first_reg,
            count,
        }]);
    }

    let specs = parameter_moves(proto)?;
    let expected_words: usize = specs.iter().map(|(words, _)| *words).sum();
    if expected_words != registers.len() {
        warn!(
            register_count = registers.len(),
            expected_words, proto, "invoke-static registers do not match prototype"
        );
        return None;
    }
    let scratch =
        find_contiguous_free_registers(code, index, registers.len(), registers).or_else(|| {
            warn!(
                register_count = registers.len(),
                index, "no contiguous scratch registers for invoke-static"
            );
            None
        })?;
    let first_reg = scratch[0];
    let mut lowered = Vec::with_capacity(specs.len() + 1);
    let mut src = 0usize;
    let mut dest = first_reg;
    for (words, kind) in specs {
        if words == 2 && registers.get(src + 1) != Some(&(registers[src] + 1)) {
            warn!(
                proto,
                "wide invoke-static argument must occupy consecutive registers"
            );
            return None;
        }
        lowered.push(build_move(kind, dest, registers[src]));
        src += words;
        dest += words as u16;
    }
    lowered.push(DexInsn::InvokeStaticRange {
        method,
        first_reg,
        count,
    });
    Some(lowered)
}

/// Register words and move kind of each parameter in `proto`.
fn parameter_moves(proto: &str) -> Option<Vec<(usize, MoveKind)>> {
    let mut params = proto.strip_prefix('(')?.split_once(')')?.0;
    let mut specs = Vec::new();
    while let Some(first) = params.chars().next() {
        let (consumed, spec) = match first {
            'L' => (params.find(';')? + 1, (1, MoveKind::Object)),
            '[' => {
                let element = params.trim_start_matches('[');
                let element_len = if element.starts_with('L') {
                    element.find(';')? + 1
                } else {
                    1
                };
                (
                    params.len() - element.len() + element_len,
                    (1, MoveKind::Object),
                )
            }
            'J' | 'D' => (1, (2, MoveKind::Wide)),
            _ => (1, (1, MoveKind::Narrow)),
        };
        specs.push(spec);
        params = &params[consumed..];
    }
    Some(specs)
}

fn build_move(kind: MoveKind, dest: u16, src: u16) -> DexInsn {
    let (d4, s4, d8) = (dest as u8, src as u8, dest as u8);
    match (kind, dest <= 15 && src <= 15, dest <= u8::MAX as u16) {
        (MoveKind::Narrow, true, _) => DexInsn::Move { dest: d4, src: s4 },
        (MoveKind::Narrow, false, true) => DexInsn::MoveFrom16 { dest: d8, src },
        (MoveKind::Narrow, false, false) => DexInsn::Move16 { dest, src },
        (MoveKind::Wide, true, _) => DexInsn::MoveWide { dest: d4, src: s4 },
        (MoveKind::Wide, false, true) => DexInsn::MoveWideFrom16 { dest: d8, src },
        (MoveKind::Wide, false, false) => DexInsn::MoveWide16 { dest, src },
        (MoveKind::Object, true, _) => DexInsn::MoveObject { dest: d4, src: s4 },
        (MoveKind::Object, false, true) => DexInsn::MoveObjectFrom16 { dest: d8, src },
        (MoveKind::Object, false, false) => DexInsn::MoveObject16 { dest, src },
    }
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
            *v = fit(value, "literal does not fit const/high16")?
        }
        DexInsn::ConstWide16 { value: v, .. } => {
            *v = fit(value, "literal does not fit const-wide/16")?
        }
        DexInsn::ConstWide32 { value: v, .. } => {
            *v = fit(value, "literal does not fit const-wide/32")?
        }
        DexInsn::ConstWide { value: v, .. } => *v = value,
        DexInsn::ConstWideHigh16 { value: v, .. } => {
            *v = fit(value, "literal does not fit const-wide/high16")?
        }
        DexInsn::AddIntLit16 { literal, .. }
        | DexInsn::RsubIntLit16 { literal, .. }
        | DexInsn::MulIntLit16 { literal, .. }
        | DexInsn::DivIntLit16 { literal, .. }
        | DexInsn::RemIntLit16 { literal, .. }
        | DexInsn::AndIntLit16 { literal, .. }
        | DexInsn::OrIntLit16 { literal, .. }
        | DexInsn::XorIntLit16 { literal, .. } => {
            *literal = fit(value, "literal does not fit lit16 opcode")?
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
            *literal = fit(value, "literal does not fit lit8 opcode")?
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
