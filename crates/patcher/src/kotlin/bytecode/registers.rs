// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Register frame queries and the one frame mutation, growing locals.

use boltffi::export;
use reseam_apk::reseam_dex::{self as dex, Instruction as DexInsn};

use crate::kotlin::handles::{code_mut, with_code, with_method_mut};

#[export]
pub fn ensure_outs_size(m: u32, min_outs_size: u16) {
    with_method_mut(m, |dex, loc| {
        let code = code_mut(dex, loc)?;
        code.outs_size = code.outs_size.max(min_outs_size);
        Some(())
    });
}

/// Adds locals below the parameter registers, renumbering parameter uses.
/// Fails when an instruction cannot encode the shifted register.
#[export]
pub fn grow_local_registers(m: u32, additional_locals: u16) -> bool {
    if additional_locals == 0 {
        return true;
    }
    with_method_mut(m, |dex, loc| {
        let code = code_mut(dex, loc)?;
        let new_registers_size = code.registers_size.checked_add(additional_locals)?;
        let param_base = code.registers_size.checked_sub(code.ins_size)?;
        let mut shifted = code.instructions.clone();
        if !shifted
            .iter_mut()
            .all(|insn| shift_parameter_registers(insn, param_base, additional_locals))
        {
            return None;
        }
        code.instructions = shifted;
        code.registers_size = new_registers_size;
        code.debug_info = None;
        Some(())
    })
    .is_some()
}

#[export]
pub fn registers_size(m: u32) -> u16 {
    with_code(m, |_, code| Some(code.registers_size)).unwrap_or(0)
}

#[export]
pub fn ins_size(m: u32) -> u16 {
    with_code(m, |_, code| Some(code.ins_size)).unwrap_or(0)
}

#[export]
pub fn outs_size(m: u32) -> u16 {
    with_code(m, |_, code| Some(code.outs_size)).unwrap_or(0)
}

#[export]
pub fn find_free_register(m: u32, at_index: u32, exclude: Vec<u16>) -> u16 {
    with_code(m, |_, code| {
        dex::find_free_register(code, at_index as usize, &exclude)
    })
    .unwrap_or(0)
}

#[export]
pub fn find_free_registers(m: u32, at_index: u32, count: u32, exclude: Vec<u16>) -> Vec<u16> {
    with_code(m, |_, code| {
        dex::find_free_registers(code, at_index as usize, count as usize, &exclude)
    })
    .unwrap_or_default()
}

#[export]
pub fn find_contiguous_free_registers(
    m: u32,
    at_index: u32,
    count: u32,
    exclude: Vec<u16>,
) -> Vec<u16> {
    with_code(m, |_, code| {
        dex::find_contiguous_free_registers(code, at_index as usize, count as usize, &exclude)
    })
    .unwrap_or_default()
}

/// The `position`th register operand of the instruction at `index`, or 0.
#[export]
pub fn instruction_register(m: u32, index: u32, position: u32) -> u16 {
    with_code(m, |_, code| {
        code.instructions
            .get(index as usize)?
            .registers_used()
            .get(position as usize)
            .copied()
    })
    .unwrap_or(0)
}

#[export]
pub fn instruction_wide_literal(m: u32, index: u32) -> i64 {
    with_code(m, |_, code| {
        code.instructions.get(index as usize)?.literal()
    })
    .unwrap_or(0)
}

fn shift_parameter_registers(insn: &mut DexInsn, param_base: u16, additional: u16) -> bool {
    match insn {
        DexInsn::Nop
        | DexInsn::ReturnVoid
        | DexInsn::Goto { .. }
        | DexInsn::Goto16 { .. }
        | DexInsn::Goto32 { .. }
        | DexInsn::PackedSwitchPayload { .. }
        | DexInsn::SparseSwitchPayload { .. }
        | DexInsn::FillArrayDataPayload { .. } => true,

        DexInsn::RawInstruction { .. } => false,

        DexInsn::Move { dest, src }
        | DexInsn::MoveWide { dest, src }
        | DexInsn::MoveObject { dest, src } => {
            shift_u4(dest, param_base, additional) && shift_u4(src, param_base, additional)
        }
        DexInsn::MoveFrom16 { dest, src }
        | DexInsn::MoveWideFrom16 { dest, src }
        | DexInsn::MoveObjectFrom16 { dest, src } => {
            shift_u8(dest, param_base, additional) && shift_u16(src, param_base, additional)
        }
        DexInsn::Move16 { dest, src }
        | DexInsn::MoveWide16 { dest, src }
        | DexInsn::MoveObject16 { dest, src } => {
            shift_u16(dest, param_base, additional) && shift_u16(src, param_base, additional)
        }

        DexInsn::MoveResult { dest }
        | DexInsn::MoveResultWide { dest }
        | DexInsn::MoveResultObject { dest }
        | DexInsn::MoveException { dest }
        | DexInsn::Const16 { dest, .. }
        | DexInsn::Const { dest, .. }
        | DexInsn::ConstHigh16 { dest, .. }
        | DexInsn::ConstWide16 { dest, .. }
        | DexInsn::ConstWide32 { dest, .. }
        | DexInsn::ConstWide { dest, .. }
        | DexInsn::ConstWideHigh16 { dest, .. }
        | DexInsn::ConstString { dest, .. }
        | DexInsn::ConstStringJumbo { dest, .. }
        | DexInsn::ConstClass { dest, .. }
        | DexInsn::NewInstance { dest, .. }
        | DexInsn::Sget { dest, .. }
        | DexInsn::SgetWide { dest, .. }
        | DexInsn::SgetObject { dest, .. }
        | DexInsn::SgetBoolean { dest, .. }
        | DexInsn::SgetByte { dest, .. }
        | DexInsn::SgetChar { dest, .. }
        | DexInsn::SgetShort { dest, .. }
        | DexInsn::ConstMethodHandle { dest, .. }
        | DexInsn::ConstMethodType { dest, .. } => shift_u8(dest, param_base, additional),

        DexInsn::Return { src }
        | DexInsn::ReturnWide { src }
        | DexInsn::ReturnObject { src }
        | DexInsn::MonitorEnter { ref_: src }
        | DexInsn::MonitorExit { ref_: src }
        | DexInsn::CheckCast { ref_: src, .. }
        | DexInsn::Throw { exception: src }
        | DexInsn::FillArrayData { array: src, .. }
        | DexInsn::PackedSwitch { test: src, .. }
        | DexInsn::SparseSwitch { test: src, .. }
        | DexInsn::Sput { src, .. }
        | DexInsn::SputWide { src, .. }
        | DexInsn::SputObject { src, .. }
        | DexInsn::SputBoolean { src, .. }
        | DexInsn::SputByte { src, .. }
        | DexInsn::SputChar { src, .. }
        | DexInsn::SputShort { src, .. } => shift_u8(src, param_base, additional),

        DexInsn::Const4 { dest, .. } => shift_u4(dest, param_base, additional),

        DexInsn::NegInt { dest, src }
        | DexInsn::NotInt { dest, src }
        | DexInsn::NegLong { dest, src }
        | DexInsn::NotLong { dest, src }
        | DexInsn::NegFloat { dest, src }
        | DexInsn::NegDouble { dest, src }
        | DexInsn::IntToLong { dest, src }
        | DexInsn::IntToFloat { dest, src }
        | DexInsn::IntToDouble { dest, src }
        | DexInsn::LongToInt { dest, src }
        | DexInsn::LongToFloat { dest, src }
        | DexInsn::LongToDouble { dest, src }
        | DexInsn::FloatToInt { dest, src }
        | DexInsn::FloatToLong { dest, src }
        | DexInsn::FloatToDouble { dest, src }
        | DexInsn::DoubleToInt { dest, src }
        | DexInsn::DoubleToLong { dest, src }
        | DexInsn::DoubleToFloat { dest, src }
        | DexInsn::IntToByte { dest, src }
        | DexInsn::IntToChar { dest, src }
        | DexInsn::IntToShort { dest, src } => {
            shift_u4(dest, param_base, additional) && shift_u4(src, param_base, additional)
        }

        DexInsn::InstanceOf { dest, ref_, .. }
        | DexInsn::ArrayLength { dest, array: ref_ }
        | DexInsn::NewArray {
            dest, size: ref_, ..
        }
        | DexInsn::Iget {
            dest, obj: ref_, ..
        }
        | DexInsn::IgetWide {
            dest, obj: ref_, ..
        }
        | DexInsn::IgetObject {
            dest, obj: ref_, ..
        }
        | DexInsn::IgetBoolean {
            dest, obj: ref_, ..
        }
        | DexInsn::IgetByte {
            dest, obj: ref_, ..
        }
        | DexInsn::IgetChar {
            dest, obj: ref_, ..
        }
        | DexInsn::IgetShort {
            dest, obj: ref_, ..
        }
        | DexInsn::AddIntLit16 {
            dest, src: ref_, ..
        }
        | DexInsn::RsubIntLit16 {
            dest, src: ref_, ..
        }
        | DexInsn::MulIntLit16 {
            dest, src: ref_, ..
        }
        | DexInsn::DivIntLit16 {
            dest, src: ref_, ..
        }
        | DexInsn::RemIntLit16 {
            dest, src: ref_, ..
        }
        | DexInsn::AndIntLit16 {
            dest, src: ref_, ..
        }
        | DexInsn::OrIntLit16 {
            dest, src: ref_, ..
        }
        | DexInsn::XorIntLit16 {
            dest, src: ref_, ..
        } => shift_u4(dest, param_base, additional) && shift_u4(ref_, param_base, additional),

        DexInsn::IfEq { a, b, .. }
        | DexInsn::IfNe { a, b, .. }
        | DexInsn::IfLt { a, b, .. }
        | DexInsn::IfGe { a, b, .. }
        | DexInsn::IfGt { a, b, .. }
        | DexInsn::IfLe { a, b, .. }
        | DexInsn::AddInt2Addr { dest_a: a, b }
        | DexInsn::SubInt2Addr { dest_a: a, b }
        | DexInsn::MulInt2Addr { dest_a: a, b }
        | DexInsn::DivInt2Addr { dest_a: a, b }
        | DexInsn::RemInt2Addr { dest_a: a, b }
        | DexInsn::AndInt2Addr { dest_a: a, b }
        | DexInsn::OrInt2Addr { dest_a: a, b }
        | DexInsn::XorInt2Addr { dest_a: a, b }
        | DexInsn::ShlInt2Addr { dest_a: a, b }
        | DexInsn::ShrInt2Addr { dest_a: a, b }
        | DexInsn::UshrInt2Addr { dest_a: a, b }
        | DexInsn::AddLong2Addr { dest_a: a, b }
        | DexInsn::SubLong2Addr { dest_a: a, b }
        | DexInsn::MulLong2Addr { dest_a: a, b }
        | DexInsn::DivLong2Addr { dest_a: a, b }
        | DexInsn::RemLong2Addr { dest_a: a, b }
        | DexInsn::AndLong2Addr { dest_a: a, b }
        | DexInsn::OrLong2Addr { dest_a: a, b }
        | DexInsn::XorLong2Addr { dest_a: a, b }
        | DexInsn::ShlLong2Addr { dest_a: a, b }
        | DexInsn::ShrLong2Addr { dest_a: a, b }
        | DexInsn::UshrLong2Addr { dest_a: a, b }
        | DexInsn::AddFloat2Addr { dest_a: a, b }
        | DexInsn::SubFloat2Addr { dest_a: a, b }
        | DexInsn::MulFloat2Addr { dest_a: a, b }
        | DexInsn::DivFloat2Addr { dest_a: a, b }
        | DexInsn::RemFloat2Addr { dest_a: a, b }
        | DexInsn::AddDouble2Addr { dest_a: a, b }
        | DexInsn::SubDouble2Addr { dest_a: a, b }
        | DexInsn::MulDouble2Addr { dest_a: a, b }
        | DexInsn::DivDouble2Addr { dest_a: a, b }
        | DexInsn::RemDouble2Addr { dest_a: a, b } => {
            shift_u4(a, param_base, additional) && shift_u4(b, param_base, additional)
        }

        DexInsn::IfEqz { a, .. }
        | DexInsn::IfNez { a, .. }
        | DexInsn::IfLtz { a, .. }
        | DexInsn::IfGez { a, .. }
        | DexInsn::IfGtz { a, .. }
        | DexInsn::IfLez { a, .. } => shift_u8(a, param_base, additional),

        DexInsn::CmpLFloat { dest, a, b }
        | DexInsn::CmpGFloat { dest, a, b }
        | DexInsn::CmpLDouble { dest, a, b }
        | DexInsn::CmpGDouble { dest, a, b }
        | DexInsn::CmpLong { dest, a, b }
        | DexInsn::AddInt { dest, a, b }
        | DexInsn::SubInt { dest, a, b }
        | DexInsn::MulInt { dest, a, b }
        | DexInsn::DivInt { dest, a, b }
        | DexInsn::RemInt { dest, a, b }
        | DexInsn::AndInt { dest, a, b }
        | DexInsn::OrInt { dest, a, b }
        | DexInsn::XorInt { dest, a, b }
        | DexInsn::ShlInt { dest, a, b }
        | DexInsn::ShrInt { dest, a, b }
        | DexInsn::UshrInt { dest, a, b }
        | DexInsn::AddLong { dest, a, b }
        | DexInsn::SubLong { dest, a, b }
        | DexInsn::MulLong { dest, a, b }
        | DexInsn::DivLong { dest, a, b }
        | DexInsn::RemLong { dest, a, b }
        | DexInsn::AndLong { dest, a, b }
        | DexInsn::OrLong { dest, a, b }
        | DexInsn::XorLong { dest, a, b }
        | DexInsn::ShlLong { dest, a, b }
        | DexInsn::ShrLong { dest, a, b }
        | DexInsn::UshrLong { dest, a, b }
        | DexInsn::AddFloat { dest, a, b }
        | DexInsn::SubFloat { dest, a, b }
        | DexInsn::MulFloat { dest, a, b }
        | DexInsn::DivFloat { dest, a, b }
        | DexInsn::RemFloat { dest, a, b }
        | DexInsn::AddDouble { dest, a, b }
        | DexInsn::SubDouble { dest, a, b }
        | DexInsn::MulDouble { dest, a, b }
        | DexInsn::DivDouble { dest, a, b }
        | DexInsn::RemDouble { dest, a, b } => {
            shift_u8(dest, param_base, additional)
                && shift_u8(a, param_base, additional)
                && shift_u8(b, param_base, additional)
        }

        DexInsn::Aget { dest, array, index }
        | DexInsn::AgetWide { dest, array, index }
        | DexInsn::AgetObject { dest, array, index }
        | DexInsn::AgetBoolean { dest, array, index }
        | DexInsn::AgetByte { dest, array, index }
        | DexInsn::AgetChar { dest, array, index }
        | DexInsn::AgetShort { dest, array, index }
        | DexInsn::Aput {
            src: dest,
            array,
            index,
        }
        | DexInsn::AputWide {
            src: dest,
            array,
            index,
        }
        | DexInsn::AputObject {
            src: dest,
            array,
            index,
        }
        | DexInsn::AputBoolean {
            src: dest,
            array,
            index,
        }
        | DexInsn::AputByte {
            src: dest,
            array,
            index,
        }
        | DexInsn::AputChar {
            src: dest,
            array,
            index,
        }
        | DexInsn::AputShort {
            src: dest,
            array,
            index,
        } => {
            shift_u8(dest, param_base, additional)
                && shift_u8(array, param_base, additional)
                && shift_u8(index, param_base, additional)
        }

        DexInsn::Iput { src, obj, .. }
        | DexInsn::IputWide { src, obj, .. }
        | DexInsn::IputObject { src, obj, .. }
        | DexInsn::IputBoolean { src, obj, .. }
        | DexInsn::IputByte { src, obj, .. }
        | DexInsn::IputChar { src, obj, .. }
        | DexInsn::IputShort { src, obj, .. } => {
            shift_u4(src, param_base, additional) && shift_u4(obj, param_base, additional)
        }

        DexInsn::AddIntLit8 { dest, src, .. }
        | DexInsn::RsubIntLit8 { dest, src, .. }
        | DexInsn::MulIntLit8 { dest, src, .. }
        | DexInsn::DivIntLit8 { dest, src, .. }
        | DexInsn::RemIntLit8 { dest, src, .. }
        | DexInsn::AndIntLit8 { dest, src, .. }
        | DexInsn::OrIntLit8 { dest, src, .. }
        | DexInsn::XorIntLit8 { dest, src, .. }
        | DexInsn::ShlIntLit8 { dest, src, .. }
        | DexInsn::ShrIntLit8 { dest, src, .. }
        | DexInsn::UshrIntLit8 { dest, src, .. } => {
            shift_u8(dest, param_base, additional) && shift_u8(src, param_base, additional)
        }

        DexInsn::FilledNewArray { args, .. }
        | DexInsn::InvokeVirtual { args, .. }
        | DexInsn::InvokeSuper { args, .. }
        | DexInsn::InvokeDirect { args, .. }
        | DexInsn::InvokeStatic { args, .. }
        | DexInsn::InvokeInterface { args, .. }
        | DexInsn::InvokePolymorphic { args, .. }
        | DexInsn::InvokeCustom { args, .. } => args
            .iter_mut()
            .all(|reg| shift_u4(reg, param_base, additional)),

        DexInsn::FilledNewArrayRange {
            first_reg, count, ..
        }
        | DexInsn::InvokeVirtualRange {
            first_reg, count, ..
        }
        | DexInsn::InvokeSuperRange {
            first_reg, count, ..
        }
        | DexInsn::InvokeDirectRange {
            first_reg, count, ..
        }
        | DexInsn::InvokeStaticRange {
            first_reg, count, ..
        }
        | DexInsn::InvokeInterfaceRange {
            first_reg, count, ..
        }
        | DexInsn::InvokePolymorphicRange {
            first_reg, count, ..
        }
        | DexInsn::InvokeCustomRange {
            first_reg, count, ..
        } => shift_range(first_reg, *count, param_base, additional),

        _ => false,
    }
}

fn shifted(register: u16, param_base: u16, additional: u16) -> Option<u16> {
    if register >= param_base {
        register.checked_add(additional)
    } else {
        Some(register)
    }
}

fn shift_u4(register: &mut u8, param_base: u16, additional: u16) -> bool {
    shift_within(register, 15, param_base, additional)
}

fn shift_u8(register: &mut u8, param_base: u16, additional: u16) -> bool {
    shift_within(register, u8::MAX as u16, param_base, additional)
}

fn shift_within(register: &mut u8, max: u16, param_base: u16, additional: u16) -> bool {
    match shifted(u16::from(*register), param_base, additional) {
        Some(value) if value <= max => {
            *register = value as u8;
            true
        }
        _ => false,
    }
}

fn shift_u16(register: &mut u16, param_base: u16, additional: u16) -> bool {
    match shifted(*register, param_base, additional) {
        Some(value) => {
            *register = value;
            true
        }
        None => false,
    }
}

/// A range must lie entirely on one side of the parameter boundary.
fn shift_range(first_reg: &mut u16, count: u8, param_base: u16, additional: u16) -> bool {
    if count == 0 {
        return true;
    }
    let last_reg = first_reg.saturating_add(u16::from(count) - 1);
    if last_reg < param_base {
        return true;
    }
    *first_reg >= param_base && shift_u16(first_reg, param_base, additional)
}
