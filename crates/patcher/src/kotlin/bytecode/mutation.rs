// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use smallvec::SmallVec;
use reseam_apk::reseam_dex::{
    find_contiguous_free_registers, CodeItem, Instruction as DexInsn, MethodIdx,
};
use tracing::warn;

use boltffi::export;

use crate::kotlin::convert::kotlin_to_dex;
use crate::kotlin::types::Instruction;
use crate::kotlin::{get_method_mut, with_ctx, with_handles};

#[export]
pub fn set_instructions(m: u32, insns: Vec<Instruction>) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let dex_insns: Vec<DexInsn> = insns.iter().map(|ki| kotlin_to_dex(ki, dex)).collect();
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        if let Some(code) = &mut method.code {
            code.set_instructions(dex_insns);
        }
    });
}

#[export]
pub fn replace_body(m: u32, registers_size: u16, outs_size: u16, insns: Vec<Instruction>) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let dex_insns: Vec<DexInsn> = insns.iter().map(|ki| kotlin_to_dex(ki, dex)).collect();
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        if let Some(code) = &mut method.code {
            code.set_instructions(dex_insns);
            code.registers_size = registers_size;
            code.outs_size = outs_size;
            code.debug_info = None;
        }
    });
}

#[export]
pub fn insert_instruction(m: u32, index: u32, insn: Instruction) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let dex_insn = kotlin_to_dex(&insn, dex);
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        if let Some(code) = &mut method.code {
            if let Err(e) = code.insert_instruction(index as usize, dex_insn) {
                warn!(error = %e, "insert_instruction failed");
            }
        }
    });
}

#[export]
pub fn insert_instructions(m: u32, index: u32, insns: Vec<Instruction>) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let dex_insns: Vec<DexInsn> = insns.iter().map(|ki| kotlin_to_dex(ki, dex)).collect();
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        if let Some(code) = &mut method.code {
            if let Err(e) = code.insert_instructions(index as usize, &dex_insns) {
                warn!(error = %e, "insert_instructions failed");
            }
        }
    });
}

#[export]
pub fn replace_instruction(m: u32, index: u32, insn: Instruction) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let dex_insn = kotlin_to_dex(&insn, dex);
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        if let Some(code) = &mut method.code {
            if let Err(e) = code.replace_instruction(index as usize, dex_insn) {
                warn!(error = %e, "replace_instruction failed");
            }
        }
    });
}

#[export]
pub fn remove_instruction(m: u32, index: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        if let Some(code) = &mut method.code {
            if let Err(e) = code.remove_instruction(index as usize) {
                warn!(error = %e, "remove_instruction failed");
            }
        }
    });
}

#[export]
pub fn remove_instructions(m: u32, index: u32, count: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        if let Some(code) = &mut method.code {
            for _ in 0..count {
                if (index as usize) < code.instructions.len() {
                    if let Err(e) = code.remove_instruction(index as usize) {
                        warn!(error = %e, "remove_instruction failed");
                        break;
                    }
                }
            }
        }
    });
}

#[export]
pub fn return_early(m: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        method.return_early();
    });
}

#[export]
pub fn return_early_int(m: u32, value: i32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        method.return_early_int(value);
    });
}

#[export]
pub fn return_early_bool(m: u32, value: bool) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        method.return_early_int(if value { 1 } else { 0 });
    });
}

#[export]
pub fn return_early_object_null(m: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        if let Some(code) = &mut method.code {
            code.set_instructions(vec![
                DexInsn::Const4 { dest: 0, value: 0 },
                DexInsn::ReturnObject { src: 0 },
            ]);
        }
    });
}

#[export]
pub fn return_early_wide(m: u32, value: i64) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        if let Some(code) = &mut method.code {
            code.set_instructions(vec![
                DexInsn::ConstWide { dest: 0, value },
                DexInsn::ReturnWide { src: 0 },
            ]);
        }
    });
}

#[export]
pub fn replace_string(m: u32, old: String, new: String) -> bool {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return false,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return false,
        };
        let old_idx = match dex.find_string_idx(&old) {
            Some(idx) => idx,
            None => return false,
        };
        let new_idx = dex.intern_string(&new);
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return false,
        };
        if let Some(code) = &mut method.code {
            for insn in &mut code.instructions {
                if insn.string_ref() == Some(old_idx) {
                    if let DexInsn::ConstString { string, .. }
                    | DexInsn::ConstStringJumbo { string, .. } = insn
                    {
                        *string = new_idx;
                        return true;
                    }
                }
            }
        }
        false
    })
}

#[export]
pub fn replace_all_strings(m: u32, old: String, new: String) -> u32 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let old_idx = match dex.find_string_idx(&old) {
            Some(idx) => idx,
            None => return 0,
        };
        let new_idx = dex.intern_string(&new);
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return 0,
        };
        let mut count = 0u32;
        if let Some(code) = &mut method.code {
            for insn in &mut code.instructions {
                if insn.string_ref() == Some(old_idx) {
                    if let DexInsn::ConstString { string, .. }
                    | DexInsn::ConstStringJumbo { string, .. } = insn
                    {
                        *string = new_idx;
                        count += 1;
                    }
                }
            }
        }
        count
    })
}

#[export]
pub fn replace_literal(m: u32, old: i64, new: i64) -> bool {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return false,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return false,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return false,
        };
        if let Some(code) = &mut method.code {
            for insn in &mut code.instructions {
                if insn.literal() == Some(old) {
                    match set_insn_literal(insn, new) {
                        Ok(()) => return true,
                        Err(message) => {
                            warn!(old, new, %message, "replace_literal failed");
                            return false;
                        }
                    }
                }
            }
        }
        false
    })
}

#[export]
pub fn replace_all_literals(m: u32, old: i64, new: i64) -> u32 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return 0,
        };
        let mut count = 0u32;
        if let Some(code) = &mut method.code {
            for insn in &mut code.instructions {
                if insn.literal() == Some(old) {
                    match set_insn_literal(insn, new) {
                        Ok(()) => count += 1,
                        Err(message) => {
                            warn!(old, new, %message, "replace_all_literals skipped unrepresentable replacement");
                        }
                    }
                }
            }
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
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return false,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return false,
        };
        let new_method_idx = match dex.intern_method(&new_class, &new_name, &new_proto) {
            Ok(idx) => idx,
            Err(_) => return false,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return false,
        };
        if let Some(code) = &mut method.code {
            if let Some(insn) = code.instructions.get_mut(index as usize) {
                match set_insn_method_ref(insn, new_method_idx) {
                    Ok(()) => return true,
                    Err(message) => {
                        warn!(index, %message, "replace_method_call failed");
                        return false;
                    }
                }
            }
        }
        false
    })
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
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return false,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return false,
        };
        let method_idx = match dex.intern_method(&class_name, &name, &proto) {
            Ok(idx) => idx,
            Err(_) => return false,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return false,
        };
        if let Some(code) = &mut method.code {
            let lowered =
                match lower_static_invoke(code, index as usize, method_idx, &proto, &registers) {
                    Some(insns) => insns,
                    None => return false,
                };
            match code.insert_instructions(index as usize, &lowered) {
                Ok(()) => return true,
                Err(e) => {
                    warn!(error = %e, "insert_invoke_static failed");
                    return false;
                }
            }
        }
        false
    })
}

#[export]
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
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return false,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return false,
        };
        let method_idx = match dex.intern_method(&class_name, &name, &proto) {
            Ok(idx) => idx,
            Err(_) => return false,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return false,
        };
        if let Some(code) = &mut method.code {
            let mut lowered =
                match lower_static_invoke(code, index as usize, method_idx, &proto, &registers) {
                    Some(insns) => insns,
                    None => return false,
                };
            let move_result = match build_move_result(result_register, is_object) {
                Some(insn) => insn,
                None => return false,
            };
            lowered.push(move_result);
            match code.insert_instructions(index as usize, &lowered) {
                Ok(()) => return true,
                Err(e) => {
                    warn!(error = %e, "insert_invoke_static_with_move_result failed");
                    return false;
                }
            }
        }
        false
    })
}

#[derive(Clone, Copy)]
enum InvokeMoveKind {
    Narrow,
    Wide,
    Object,
}

fn lower_static_invoke(
    code: &CodeItem,
    index: usize,
    method: MethodIdx,
    proto: &str,
    registers: &[u16],
) -> Option<Vec<DexInsn>> {
    if registers.len() > u8::MAX as usize {
        warn!(
            register_count = registers.len(),
            "invoke-static register count exceeds range encoding capacity"
        );
        return None;
    }

    if invoke_is_directly_encodable(registers) {
        let args: SmallVec<[u8; 5]> = registers.iter().map(|r| *r as u8).collect();
        return Some(vec![DexInsn::InvokeStatic { method, args }]);
    }

    if invoke_registers_are_consecutive(registers) {
        return Some(vec![DexInsn::InvokeStaticRange {
            method,
            first_reg: registers.first().copied().unwrap_or(0),
            count: registers.len() as u8,
        }]);
    }

    let specs = parse_static_invoke_arg_specs(proto)?;
    let expected_words: usize = specs.iter().map(|(words, _)| *words).sum();
    if expected_words != registers.len() {
        warn!(
            register_count = registers.len(),
            expected_words,
            proto,
            "invoke-static registers do not match method prototype"
        );
        return None;
    }

    let scratch = match find_contiguous_free_registers(code, index, registers.len(), registers) {
        Some(regs) => regs,
        None => {
            warn!(
                register_count = registers.len(),
                index,
                "no contiguous scratch registers available for invoke-static lowering"
            );
            return None;
        }
    };
    let scratch_start = scratch.first().copied().unwrap_or(0);

    let mut lowered = Vec::with_capacity(specs.len() + 1);
    let mut src_index = 0usize;
    let mut dest = scratch_start;
    for (word_count, kind) in specs {
        let src = registers[src_index];
        if word_count == 2 {
            let Some(&src_hi) = registers.get(src_index + 1) else {
                warn!(proto, "missing second register word for wide invoke-static arg");
                return None;
            };
            if src_hi != src + 1 {
                warn!(
                    src,
                    src_hi,
                    proto,
                    "wide invoke-static arg must occupy consecutive source registers"
                );
                return None;
            }
        }
        lowered.push(build_move(kind, dest, src));
        src_index += word_count;
        dest += word_count as u16;
    }
    lowered.push(DexInsn::InvokeStaticRange {
        method,
        first_reg: scratch_start,
        count: registers.len() as u8,
    });
    Some(lowered)
}

fn parse_static_invoke_arg_specs(proto: &str) -> Option<Vec<(usize, InvokeMoveKind)>> {
    let params = proto.strip_prefix('(')?.split_once(')')?.0;
    let mut specs = Vec::new();
    let bytes = params.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] as char {
            '[' => {
                i += 1;
                while i < bytes.len() && bytes[i] as char == '[' {
                    i += 1;
                }
                if i >= bytes.len() {
                    return None;
                }
                if bytes[i] as char == 'L' {
                    while i < bytes.len() && bytes[i] as char != ';' {
                        i += 1;
                    }
                    if i >= bytes.len() {
                        return None;
                    }
                }
                i += 1;
                specs.push((1, InvokeMoveKind::Object));
            }
            'L' => {
                while i < bytes.len() && bytes[i] as char != ';' {
                    i += 1;
                }
                if i >= bytes.len() {
                    return None;
                }
                i += 1;
                specs.push((1, InvokeMoveKind::Object));
            }
            'J' | 'D' => {
                i += 1;
                specs.push((2, InvokeMoveKind::Wide));
            }
            _ => {
                i += 1;
                specs.push((1, InvokeMoveKind::Narrow));
            }
        }
    }
    Some(specs)
}

fn invoke_is_directly_encodable(registers: &[u16]) -> bool {
    registers.len() <= 5 && registers.iter().all(|&reg| reg <= 15)
}

fn invoke_registers_are_consecutive(registers: &[u16]) -> bool {
    registers
        .windows(2)
        .all(|pair| pair[1] == pair[0].saturating_add(1))
}

fn build_move(kind: InvokeMoveKind, dest: u16, src: u16) -> DexInsn {
    match kind {
        InvokeMoveKind::Narrow if dest <= 15 && src <= 15 => DexInsn::Move {
            dest: dest as u8,
            src: src as u8,
        },
        InvokeMoveKind::Narrow if dest <= u8::MAX as u16 => DexInsn::MoveFrom16 {
            dest: dest as u8,
            src,
        },
        InvokeMoveKind::Narrow => DexInsn::Move16 { dest, src },
        InvokeMoveKind::Wide if dest <= 15 && src <= 15 => DexInsn::MoveWide {
            dest: dest as u8,
            src: src as u8,
        },
        InvokeMoveKind::Wide if dest <= u8::MAX as u16 => DexInsn::MoveWideFrom16 {
            dest: dest as u8,
            src,
        },
        InvokeMoveKind::Wide => DexInsn::MoveWide16 { dest, src },
        InvokeMoveKind::Object if dest <= 15 && src <= 15 => DexInsn::MoveObject {
            dest: dest as u8,
            src: src as u8,
        },
        InvokeMoveKind::Object if dest <= u8::MAX as u16 => DexInsn::MoveObjectFrom16 {
            dest: dest as u8,
            src,
        },
        InvokeMoveKind::Object => DexInsn::MoveObject16 { dest, src },
    }
}

fn build_move_result(result_register: u16, is_object: bool) -> Option<DexInsn> {
    if result_register > u8::MAX as u16 {
        warn!(
            result_register,
            "move-result destination exceeds v255 and cannot be encoded"
        );
        return None;
    }

    Some(if is_object {
        DexInsn::MoveResultObject {
            dest: result_register as u8,
        }
    } else {
        DexInsn::MoveResult {
            dest: result_register as u8,
        }
    })
}

fn set_insn_literal(insn: &mut DexInsn, value: i64) -> std::result::Result<(), &'static str> {
    match insn {
        DexInsn::Const4 { value: v, .. } => *v = i8::try_from(value).map_err(|_| "literal does not fit const/4")?,
        DexInsn::Const16 { value: v, .. } => *v = i16::try_from(value).map_err(|_| "literal does not fit const/16")?,
        DexInsn::Const { value: v, .. } => *v = i32::try_from(value).map_err(|_| "literal does not fit const")?,
        DexInsn::ConstHigh16 { value: v, .. } => *v = i16::try_from(value).map_err(|_| "literal does not fit const/high16")?,
        DexInsn::ConstWide16 { value: v, .. } => *v = i16::try_from(value).map_err(|_| "literal does not fit const-wide/16")?,
        DexInsn::ConstWide32 { value: v, .. } => *v = i32::try_from(value).map_err(|_| "literal does not fit const-wide/32")?,
        DexInsn::ConstWide { value: v, .. } => *v = value,
        DexInsn::ConstWideHigh16 { value: v, .. } => *v = i16::try_from(value).map_err(|_| "literal does not fit const-wide/high16")?,
        DexInsn::AddIntLit16 { literal, .. }
        | DexInsn::RsubIntLit16 { literal, .. }
        | DexInsn::MulIntLit16 { literal, .. }
        | DexInsn::DivIntLit16 { literal, .. }
        | DexInsn::RemIntLit16 { literal, .. }
        | DexInsn::AndIntLit16 { literal, .. }
        | DexInsn::OrIntLit16 { literal, .. }
        | DexInsn::XorIntLit16 { literal, .. } => *literal = i16::try_from(value).map_err(|_| "literal does not fit lit16 opcode")?,
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
        | DexInsn::UshrIntLit8 { literal, .. } => *literal = i8::try_from(value).map_err(|_| "literal does not fit lit8 opcode")?,
        _ => return Err("instruction does not carry a writable literal"),
    }
    Ok(())
}

fn set_insn_method_ref(
    insn: &mut DexInsn,
    new_idx: MethodIdx,
) -> std::result::Result<(), &'static str> {
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
            *method = new_idx;
            Ok(())
        }
        _ => Err("target instruction is not an invoke"),
    }
}
