// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::types::instruction::Instruction;

mod access;
mod basic;
mod invoke;
mod ops;
mod payloads;

pub fn encode_instructions(instructions: &[Instruction]) -> Result<Vec<u16>> {
    let capacity = instructions
        .iter()
        .map(|instruction| instruction.code_units() as usize)
        .sum();
    let mut code = Vec::with_capacity(capacity);
    for instruction in instructions {
        encode_instruction(&mut code, instruction)?;
    }
    Ok(code)
}

fn encode_instruction(code: &mut Vec<u16>, instruction: &Instruction) -> Result<()> {
    match instruction {
        Instruction::Nop
        | Instruction::Move { .. }
        | Instruction::MoveWide { .. }
        | Instruction::MoveObject { .. }
        | Instruction::MoveFrom16 { .. }
        | Instruction::MoveWideFrom16 { .. }
        | Instruction::MoveObjectFrom16 { .. }
        | Instruction::Move16 { .. }
        | Instruction::MoveWide16 { .. }
        | Instruction::MoveObject16 { .. }
        | Instruction::MoveResult { .. }
        | Instruction::MoveResultWide { .. }
        | Instruction::MoveResultObject { .. }
        | Instruction::MoveException { .. }
        | Instruction::ReturnVoid
        | Instruction::Return { .. }
        | Instruction::ReturnWide { .. }
        | Instruction::ReturnObject { .. }
        | Instruction::Const4 { .. }
        | Instruction::Const16 { .. }
        | Instruction::Const { .. }
        | Instruction::ConstHigh16 { .. }
        | Instruction::ConstWide16 { .. }
        | Instruction::ConstWide32 { .. }
        | Instruction::ConstWide { .. }
        | Instruction::ConstWideHigh16 { .. }
        | Instruction::ConstString { .. }
        | Instruction::ConstStringJumbo { .. }
        | Instruction::ConstClass { .. }
        | Instruction::MonitorEnter { .. }
        | Instruction::MonitorExit { .. }
        | Instruction::CheckCast { .. }
        | Instruction::InstanceOf { .. }
        | Instruction::ArrayLength { .. }
        | Instruction::NewInstance { .. }
        | Instruction::NewArray { .. }
        | Instruction::FillArrayData { .. }
        | Instruction::Throw { .. }
        | Instruction::Goto { .. }
        | Instruction::Goto16 { .. }
        | Instruction::Goto32 { .. }
        | Instruction::PackedSwitch { .. }
        | Instruction::SparseSwitch { .. }
        | Instruction::IfEq { .. }
        | Instruction::IfNe { .. }
        | Instruction::IfLt { .. }
        | Instruction::IfGe { .. }
        | Instruction::IfGt { .. }
        | Instruction::IfLe { .. }
        | Instruction::IfEqz { .. }
        | Instruction::IfNez { .. }
        | Instruction::IfLtz { .. }
        | Instruction::IfGez { .. }
        | Instruction::IfGtz { .. }
        | Instruction::IfLez { .. }
        | Instruction::ConstMethodHandle { .. }
        | Instruction::ConstMethodType { .. } => basic::encode_instruction(code, instruction),

        Instruction::Aget { .. }
        | Instruction::AgetWide { .. }
        | Instruction::AgetObject { .. }
        | Instruction::AgetBoolean { .. }
        | Instruction::AgetByte { .. }
        | Instruction::AgetChar { .. }
        | Instruction::AgetShort { .. }
        | Instruction::Aput { .. }
        | Instruction::AputWide { .. }
        | Instruction::AputObject { .. }
        | Instruction::AputBoolean { .. }
        | Instruction::AputByte { .. }
        | Instruction::AputChar { .. }
        | Instruction::AputShort { .. }
        | Instruction::Iget { .. }
        | Instruction::IgetWide { .. }
        | Instruction::IgetObject { .. }
        | Instruction::IgetBoolean { .. }
        | Instruction::IgetByte { .. }
        | Instruction::IgetChar { .. }
        | Instruction::IgetShort { .. }
        | Instruction::Iput { .. }
        | Instruction::IputWide { .. }
        | Instruction::IputObject { .. }
        | Instruction::IputBoolean { .. }
        | Instruction::IputByte { .. }
        | Instruction::IputChar { .. }
        | Instruction::IputShort { .. }
        | Instruction::Sget { .. }
        | Instruction::SgetWide { .. }
        | Instruction::SgetObject { .. }
        | Instruction::SgetBoolean { .. }
        | Instruction::SgetByte { .. }
        | Instruction::SgetChar { .. }
        | Instruction::SgetShort { .. }
        | Instruction::Sput { .. }
        | Instruction::SputWide { .. }
        | Instruction::SputObject { .. }
        | Instruction::SputBoolean { .. }
        | Instruction::SputByte { .. }
        | Instruction::SputChar { .. }
        | Instruction::SputShort { .. } => access::encode_instruction(code, instruction),

        Instruction::FilledNewArray { .. }
        | Instruction::FilledNewArrayRange { .. }
        | Instruction::InvokeVirtual { .. }
        | Instruction::InvokeSuper { .. }
        | Instruction::InvokeDirect { .. }
        | Instruction::InvokeStatic { .. }
        | Instruction::InvokeInterface { .. }
        | Instruction::InvokeVirtualRange { .. }
        | Instruction::InvokeSuperRange { .. }
        | Instruction::InvokeDirectRange { .. }
        | Instruction::InvokeStaticRange { .. }
        | Instruction::InvokeInterfaceRange { .. }
        | Instruction::InvokePolymorphic { .. }
        | Instruction::InvokePolymorphicRange { .. }
        | Instruction::InvokeCustom { .. }
        | Instruction::InvokeCustomRange { .. } => invoke::encode_instruction(code, instruction),

        Instruction::CmpLFloat { .. }
        | Instruction::CmpGFloat { .. }
        | Instruction::CmpLDouble { .. }
        | Instruction::CmpGDouble { .. }
        | Instruction::CmpLong { .. }
        | Instruction::NegInt { .. }
        | Instruction::NotInt { .. }
        | Instruction::NegLong { .. }
        | Instruction::NotLong { .. }
        | Instruction::NegFloat { .. }
        | Instruction::NegDouble { .. }
        | Instruction::IntToLong { .. }
        | Instruction::IntToFloat { .. }
        | Instruction::IntToDouble { .. }
        | Instruction::LongToInt { .. }
        | Instruction::LongToFloat { .. }
        | Instruction::LongToDouble { .. }
        | Instruction::FloatToInt { .. }
        | Instruction::FloatToLong { .. }
        | Instruction::FloatToDouble { .. }
        | Instruction::DoubleToInt { .. }
        | Instruction::DoubleToLong { .. }
        | Instruction::DoubleToFloat { .. }
        | Instruction::IntToByte { .. }
        | Instruction::IntToChar { .. }
        | Instruction::IntToShort { .. }
        | Instruction::AddInt { .. }
        | Instruction::SubInt { .. }
        | Instruction::MulInt { .. }
        | Instruction::DivInt { .. }
        | Instruction::RemInt { .. }
        | Instruction::AndInt { .. }
        | Instruction::OrInt { .. }
        | Instruction::XorInt { .. }
        | Instruction::ShlInt { .. }
        | Instruction::ShrInt { .. }
        | Instruction::UshrInt { .. }
        | Instruction::AddLong { .. }
        | Instruction::SubLong { .. }
        | Instruction::MulLong { .. }
        | Instruction::DivLong { .. }
        | Instruction::RemLong { .. }
        | Instruction::AndLong { .. }
        | Instruction::OrLong { .. }
        | Instruction::XorLong { .. }
        | Instruction::ShlLong { .. }
        | Instruction::ShrLong { .. }
        | Instruction::UshrLong { .. }
        | Instruction::AddFloat { .. }
        | Instruction::SubFloat { .. }
        | Instruction::MulFloat { .. }
        | Instruction::DivFloat { .. }
        | Instruction::RemFloat { .. }
        | Instruction::AddDouble { .. }
        | Instruction::SubDouble { .. }
        | Instruction::MulDouble { .. }
        | Instruction::DivDouble { .. }
        | Instruction::RemDouble { .. }
        | Instruction::AddInt2Addr { .. }
        | Instruction::SubInt2Addr { .. }
        | Instruction::MulInt2Addr { .. }
        | Instruction::DivInt2Addr { .. }
        | Instruction::RemInt2Addr { .. }
        | Instruction::AndInt2Addr { .. }
        | Instruction::OrInt2Addr { .. }
        | Instruction::XorInt2Addr { .. }
        | Instruction::ShlInt2Addr { .. }
        | Instruction::ShrInt2Addr { .. }
        | Instruction::UshrInt2Addr { .. }
        | Instruction::AddLong2Addr { .. }
        | Instruction::SubLong2Addr { .. }
        | Instruction::MulLong2Addr { .. }
        | Instruction::DivLong2Addr { .. }
        | Instruction::RemLong2Addr { .. }
        | Instruction::AndLong2Addr { .. }
        | Instruction::OrLong2Addr { .. }
        | Instruction::XorLong2Addr { .. }
        | Instruction::ShlLong2Addr { .. }
        | Instruction::ShrLong2Addr { .. }
        | Instruction::UshrLong2Addr { .. }
        | Instruction::AddFloat2Addr { .. }
        | Instruction::SubFloat2Addr { .. }
        | Instruction::MulFloat2Addr { .. }
        | Instruction::DivFloat2Addr { .. }
        | Instruction::RemFloat2Addr { .. }
        | Instruction::AddDouble2Addr { .. }
        | Instruction::SubDouble2Addr { .. }
        | Instruction::MulDouble2Addr { .. }
        | Instruction::DivDouble2Addr { .. }
        | Instruction::RemDouble2Addr { .. }
        | Instruction::AddIntLit16 { .. }
        | Instruction::RsubIntLit16 { .. }
        | Instruction::MulIntLit16 { .. }
        | Instruction::DivIntLit16 { .. }
        | Instruction::RemIntLit16 { .. }
        | Instruction::AndIntLit16 { .. }
        | Instruction::OrIntLit16 { .. }
        | Instruction::XorIntLit16 { .. }
        | Instruction::AddIntLit8 { .. }
        | Instruction::RsubIntLit8 { .. }
        | Instruction::MulIntLit8 { .. }
        | Instruction::DivIntLit8 { .. }
        | Instruction::RemIntLit8 { .. }
        | Instruction::AndIntLit8 { .. }
        | Instruction::OrIntLit8 { .. }
        | Instruction::XorIntLit8 { .. }
        | Instruction::ShlIntLit8 { .. }
        | Instruction::ShrIntLit8 { .. }
        | Instruction::UshrIntLit8 { .. } => ops::encode_instruction(code, instruction),

        Instruction::PackedSwitchPayload { .. }
        | Instruction::SparseSwitchPayload { .. }
        | Instruction::FillArrayDataPayload { .. }
        | Instruction::RawInstruction { .. } => payloads::encode_instruction(code, instruction),
    }
}

pub(super) fn pack_aa_op(op: u16, aa: u8) -> u16 {
    op | ((aa as u16) << 8)
}

pub(super) fn pack_12x(op: u16, a: u8, b: u8) -> u16 {
    op | ((a as u16 & 0xF) << 8) | ((b as u16 & 0xF) << 12)
}

pub(super) fn encode_23x(code: &mut Vec<u16>, op: u16, aa: u8, bb: u8, cc: u8) {
    code.push(op | ((aa as u16) << 8));
    code.push((bb as u16) | ((cc as u16) << 8));
}

pub(super) fn encode_35c(code: &mut Vec<u16>, op: u16, idx: u16, args: &[u8]) -> Result<()> {
    validate_35c_args(args)?;
    let count = args.len() as u8;
    let (c, d, e, f, g) = unpack_args(args);
    code.push(op | ((count as u16) << 12) | ((g as u16) << 8));
    code.push(idx);
    code.push((c as u16) | ((d as u16) << 4) | ((e as u16) << 8) | ((f as u16) << 12));
    Ok(())
}

pub(super) fn validate_35c_args(args: &[u8]) -> Result<()> {
    if args.len() > 5 {
        return Err(crate::error::invalid(
            "instruction",
            format!(
                "register count {} exceeds maximum 5 for format 35c/45cc — \
                 use the range variant instead",
                args.len()
            ),
        ));
    }
    if let Some(&register) = args.iter().find(|&&register| register > 15) {
        return Err(crate::error::invalid(
            "instruction",
            format!(
                "register v{register} exceeds nibble range (0-15) for format 35c/45cc — \
                 use the range variant instead"
            ),
        ));
    }
    Ok(())
}

pub(super) fn unpack_args(args: &[u8]) -> (u8, u8, u8, u8, u8) {
    let c = args.first().copied().unwrap_or(0);
    let d = args.get(1).copied().unwrap_or(0);
    let e = args.get(2).copied().unwrap_or(0);
    let f = args.get(3).copied().unwrap_or(0);
    let g = args.get(4).copied().unwrap_or(0);
    (c, d, e, f, g)
}
