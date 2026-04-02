use smallvec::SmallVec;
use stitch_apk::stitch_dex::{Instruction as DexInsn, MethodIdx};
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
        if let Some(code) = &mut method.code {
            code.set_instructions(dex_insns);
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return false };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return 0 };
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return false };
        if let Some(code) = &mut method.code {
            for insn in &mut code.instructions {
                if insn.literal() == Some(old) {
                    set_insn_literal(insn, new);
                    return true;
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return 0 };
        let mut count = 0u32;
        if let Some(code) = &mut method.code {
            for insn in &mut code.instructions {
                if insn.literal() == Some(old) {
                    set_insn_literal(insn, new);
                    count += 1;
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
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return false };
        if let Some(code) = &mut method.code {
            if let Some(insn) = code.instructions.get_mut(index as usize) {
                set_insn_method_ref(insn, new_method_idx);
                return true;
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
        let needs_range = registers.len() > 5
            || registers.iter().any(|&r| r > 15);
        let invoke = if needs_range {
            DexInsn::InvokeStaticRange {
                method: method_idx,
                first_reg: registers.first().copied().unwrap_or(0),
                count: registers.len() as u8,
            }
        } else {
            let regs: SmallVec<[u8; 5]> = registers.iter().map(|r| *r as u8).collect();
            DexInsn::InvokeStatic {
                method: method_idx,
                args: regs,
            }
        };
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return false };
        if let Some(code) = &mut method.code {
            match code.insert_instruction(index as usize, invoke) {
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
        let needs_range = registers.len() > 5
            || registers.iter().any(|&r| r > 15);
        let invoke = if needs_range {
            DexInsn::InvokeStaticRange {
                method: method_idx,
                first_reg: registers.first().copied().unwrap_or(0),
                count: registers.len() as u8,
            }
        } else {
            let regs: SmallVec<[u8; 5]> = registers.iter().map(|r| *r as u8).collect();
            DexInsn::InvokeStatic {
                method: method_idx,
                args: regs,
            }
        };
        let move_result = if is_object {
            DexInsn::MoveResultObject {
                dest: result_register as u8,
            }
        } else {
            DexInsn::MoveResult {
                dest: result_register as u8,
            }
        };
        let method = match get_method_mut(dex, mh) { Some(m) => m, None => return false };
        if let Some(code) = &mut method.code {
            match code.insert_instructions(index as usize, &[invoke, move_result]) {
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

fn set_insn_literal(insn: &mut DexInsn, value: i64) {
    match insn {
        DexInsn::Const4 { value: v, .. } => *v = value as i8,
        DexInsn::Const16 { value: v, .. } => *v = value as i16,
        DexInsn::Const { value: v, .. } => *v = value as i32,
        DexInsn::ConstHigh16 { value: v, .. } => *v = value as i16,
        DexInsn::ConstWide16 { value: v, .. } => *v = value as i16,
        DexInsn::ConstWide32 { value: v, .. } => *v = value as i32,
        DexInsn::ConstWide { value: v, .. } => *v = value,
        DexInsn::ConstWideHigh16 { value: v, .. } => *v = value as i16,
        DexInsn::AddIntLit16 { literal, .. }
        | DexInsn::RsubIntLit16 { literal, .. }
        | DexInsn::MulIntLit16 { literal, .. }
        | DexInsn::DivIntLit16 { literal, .. }
        | DexInsn::RemIntLit16 { literal, .. }
        | DexInsn::AndIntLit16 { literal, .. }
        | DexInsn::OrIntLit16 { literal, .. }
        | DexInsn::XorIntLit16 { literal, .. } => *literal = value as i16,
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
        | DexInsn::UshrIntLit8 { literal, .. } => *literal = value as i8,
        _ => {}
    }
}

fn set_insn_method_ref(insn: &mut DexInsn, new_idx: MethodIdx) {
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
        }
        _ => {}
    }
}
