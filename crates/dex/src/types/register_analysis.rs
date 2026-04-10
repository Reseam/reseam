use std::collections::HashSet;

use super::code::CodeItem;
use super::instruction::Instruction;

pub fn find_free_register(code: &CodeItem, at_index: usize, exclude: &[u16]) -> Option<u16> {
    let live = live_registers(code, at_index);
    let exclude_set: HashSet<u16> = exclude.iter().copied().collect();
    (0..code.registers_size).find(|r| !live.contains(r) && !exclude_set.contains(r))
}

pub fn find_free_registers(
    code: &CodeItem,
    at_index: usize,
    count: usize,
    exclude: &[u16],
) -> Option<Vec<u16>> {
    let live = live_registers(code, at_index);
    let exclude_set: HashSet<u16> = exclude.iter().copied().collect();
    let free: Vec<u16> = (0..code.registers_size)
        .filter(|r| !live.contains(r) && !exclude_set.contains(r))
        .take(count)
        .collect();
    if free.len() == count {
        Some(free)
    } else {
        None
    }
}

fn live_registers(code: &CodeItem, at_index: usize) -> HashSet<u16> {
    let mut live = HashSet::new();
    let len = code.instructions.len();
    if at_index >= len {
        return live;
    }

    for i in (0..at_index).rev() {
        let insn = &code.instructions[i];
        for r in written_registers(insn) {
            live.insert(r);
        }
    }

    let mut written_fwd: HashSet<u16> = HashSet::new();
    for i in at_index..len {
        let insn = &code.instructions[i];
        for r in read_registers(insn) {
            if !written_fwd.contains(&r) {
                live.insert(r);
            }
        }
        for r in written_registers(insn) {
            written_fwd.insert(r);
        }
    }

    live
}

fn written_registers(insn: &Instruction) -> Vec<u16> {
    match insn {
        Instruction::Move { dest, .. }
        | Instruction::MoveWide { dest, .. }
        | Instruction::MoveObject { dest, .. } => vec![*dest as u16],

        Instruction::MoveFrom16 { dest, .. }
        | Instruction::MoveWideFrom16 { dest, .. }
        | Instruction::MoveObjectFrom16 { dest, .. }
        | Instruction::MoveResult { dest }
        | Instruction::MoveResultWide { dest }
        | Instruction::MoveResultObject { dest }
        | Instruction::MoveException { dest }
        | Instruction::Const16 { dest, .. }
        | Instruction::Const { dest, .. }
        | Instruction::ConstHigh16 { dest, .. }
        | Instruction::ConstWide16 { dest, .. }
        | Instruction::ConstWide32 { dest, .. }
        | Instruction::ConstWide { dest, .. }
        | Instruction::ConstWideHigh16 { dest, .. }
        | Instruction::ConstString { dest, .. }
        | Instruction::ConstStringJumbo { dest, .. }
        | Instruction::ConstClass { dest, .. }
        | Instruction::NewInstance { dest, .. }
        | Instruction::ConstMethodHandle { dest, .. }
        | Instruction::ConstMethodType { dest, .. }
        | Instruction::Sget { dest, .. }
        | Instruction::SgetWide { dest, .. }
        | Instruction::SgetObject { dest, .. }
        | Instruction::SgetBoolean { dest, .. }
        | Instruction::SgetByte { dest, .. }
        | Instruction::SgetChar { dest, .. }
        | Instruction::SgetShort { dest, .. }
        | Instruction::CmpLFloat { dest, .. }
        | Instruction::CmpGFloat { dest, .. }
        | Instruction::CmpLDouble { dest, .. }
        | Instruction::CmpGDouble { dest, .. }
        | Instruction::CmpLong { dest, .. }
        | Instruction::AddInt { dest, .. }
        | Instruction::SubInt { dest, .. }
        | Instruction::MulInt { dest, .. }
        | Instruction::DivInt { dest, .. }
        | Instruction::RemInt { dest, .. }
        | Instruction::AndInt { dest, .. }
        | Instruction::OrInt { dest, .. }
        | Instruction::XorInt { dest, .. }
        | Instruction::ShlInt { dest, .. }
        | Instruction::ShrInt { dest, .. }
        | Instruction::UshrInt { dest, .. }
        | Instruction::AddLong { dest, .. }
        | Instruction::SubLong { dest, .. }
        | Instruction::MulLong { dest, .. }
        | Instruction::DivLong { dest, .. }
        | Instruction::RemLong { dest, .. }
        | Instruction::AndLong { dest, .. }
        | Instruction::OrLong { dest, .. }
        | Instruction::XorLong { dest, .. }
        | Instruction::ShlLong { dest, .. }
        | Instruction::ShrLong { dest, .. }
        | Instruction::UshrLong { dest, .. }
        | Instruction::AddFloat { dest, .. }
        | Instruction::SubFloat { dest, .. }
        | Instruction::MulFloat { dest, .. }
        | Instruction::DivFloat { dest, .. }
        | Instruction::RemFloat { dest, .. }
        | Instruction::AddDouble { dest, .. }
        | Instruction::SubDouble { dest, .. }
        | Instruction::MulDouble { dest, .. }
        | Instruction::DivDouble { dest, .. }
        | Instruction::RemDouble { dest, .. }
        | Instruction::Aget { dest, .. }
        | Instruction::AgetWide { dest, .. }
        | Instruction::AgetObject { dest, .. }
        | Instruction::AgetBoolean { dest, .. }
        | Instruction::AgetByte { dest, .. }
        | Instruction::AgetChar { dest, .. }
        | Instruction::AgetShort { dest, .. }
        | Instruction::AddIntLit8 { dest, .. }
        | Instruction::RsubIntLit8 { dest, .. }
        | Instruction::MulIntLit8 { dest, .. }
        | Instruction::DivIntLit8 { dest, .. }
        | Instruction::RemIntLit8 { dest, .. }
        | Instruction::AndIntLit8 { dest, .. }
        | Instruction::OrIntLit8 { dest, .. }
        | Instruction::XorIntLit8 { dest, .. }
        | Instruction::ShlIntLit8 { dest, .. }
        | Instruction::ShrIntLit8 { dest, .. }
        | Instruction::UshrIntLit8 { dest, .. } => vec![*dest as u16],

        Instruction::Move16 { dest, .. }
        | Instruction::MoveWide16 { dest, .. }
        | Instruction::MoveObject16 { dest, .. } => vec![*dest],

        Instruction::Const4 { dest, .. }
        | Instruction::InstanceOf { dest, .. }
        | Instruction::ArrayLength { dest, .. }
        | Instruction::NewArray { dest, .. }
        | Instruction::NegInt { dest, .. }
        | Instruction::NotInt { dest, .. }
        | Instruction::NegLong { dest, .. }
        | Instruction::NotLong { dest, .. }
        | Instruction::NegFloat { dest, .. }
        | Instruction::NegDouble { dest, .. }
        | Instruction::IntToLong { dest, .. }
        | Instruction::IntToFloat { dest, .. }
        | Instruction::IntToDouble { dest, .. }
        | Instruction::LongToInt { dest, .. }
        | Instruction::LongToFloat { dest, .. }
        | Instruction::LongToDouble { dest, .. }
        | Instruction::FloatToInt { dest, .. }
        | Instruction::FloatToLong { dest, .. }
        | Instruction::FloatToDouble { dest, .. }
        | Instruction::DoubleToInt { dest, .. }
        | Instruction::DoubleToLong { dest, .. }
        | Instruction::DoubleToFloat { dest, .. }
        | Instruction::IntToByte { dest, .. }
        | Instruction::IntToChar { dest, .. }
        | Instruction::IntToShort { dest, .. }
        | Instruction::Iget { dest, .. }
        | Instruction::IgetWide { dest, .. }
        | Instruction::IgetObject { dest, .. }
        | Instruction::IgetBoolean { dest, .. }
        | Instruction::IgetByte { dest, .. }
        | Instruction::IgetChar { dest, .. }
        | Instruction::IgetShort { dest, .. }
        | Instruction::AddIntLit16 { dest, .. }
        | Instruction::RsubIntLit16 { dest, .. }
        | Instruction::MulIntLit16 { dest, .. }
        | Instruction::DivIntLit16 { dest, .. }
        | Instruction::RemIntLit16 { dest, .. }
        | Instruction::AndIntLit16 { dest, .. }
        | Instruction::OrIntLit16 { dest, .. }
        | Instruction::XorIntLit16 { dest, .. } => vec![*dest as u16],

        Instruction::AddInt2Addr { dest_a, .. }
        | Instruction::SubInt2Addr { dest_a, .. }
        | Instruction::MulInt2Addr { dest_a, .. }
        | Instruction::DivInt2Addr { dest_a, .. }
        | Instruction::RemInt2Addr { dest_a, .. }
        | Instruction::AndInt2Addr { dest_a, .. }
        | Instruction::OrInt2Addr { dest_a, .. }
        | Instruction::XorInt2Addr { dest_a, .. }
        | Instruction::ShlInt2Addr { dest_a, .. }
        | Instruction::ShrInt2Addr { dest_a, .. }
        | Instruction::UshrInt2Addr { dest_a, .. }
        | Instruction::AddLong2Addr { dest_a, .. }
        | Instruction::SubLong2Addr { dest_a, .. }
        | Instruction::MulLong2Addr { dest_a, .. }
        | Instruction::DivLong2Addr { dest_a, .. }
        | Instruction::RemLong2Addr { dest_a, .. }
        | Instruction::AndLong2Addr { dest_a, .. }
        | Instruction::OrLong2Addr { dest_a, .. }
        | Instruction::XorLong2Addr { dest_a, .. }
        | Instruction::ShlLong2Addr { dest_a, .. }
        | Instruction::ShrLong2Addr { dest_a, .. }
        | Instruction::UshrLong2Addr { dest_a, .. }
        | Instruction::AddFloat2Addr { dest_a, .. }
        | Instruction::SubFloat2Addr { dest_a, .. }
        | Instruction::MulFloat2Addr { dest_a, .. }
        | Instruction::DivFloat2Addr { dest_a, .. }
        | Instruction::RemFloat2Addr { dest_a, .. }
        | Instruction::AddDouble2Addr { dest_a, .. }
        | Instruction::SubDouble2Addr { dest_a, .. }
        | Instruction::MulDouble2Addr { dest_a, .. }
        | Instruction::DivDouble2Addr { dest_a, .. }
        | Instruction::RemDouble2Addr { dest_a, .. } => vec![*dest_a as u16],

        Instruction::Nop
        | Instruction::ReturnVoid
        | Instruction::Return { .. }
        | Instruction::ReturnWide { .. }
        | Instruction::ReturnObject { .. }
        | Instruction::MonitorEnter { .. }
        | Instruction::MonitorExit { .. }
        | Instruction::CheckCast { .. }
        | Instruction::Throw { .. }
        | Instruction::Goto { .. }
        | Instruction::Goto16 { .. }
        | Instruction::Goto32 { .. }
        | Instruction::PackedSwitch { .. }
        | Instruction::SparseSwitch { .. }
        | Instruction::FillArrayData { .. }
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
        | Instruction::Aput { .. }
        | Instruction::AputWide { .. }
        | Instruction::AputObject { .. }
        | Instruction::AputBoolean { .. }
        | Instruction::AputByte { .. }
        | Instruction::AputChar { .. }
        | Instruction::AputShort { .. }
        | Instruction::Iput { .. }
        | Instruction::IputWide { .. }
        | Instruction::IputObject { .. }
        | Instruction::IputBoolean { .. }
        | Instruction::IputByte { .. }
        | Instruction::IputChar { .. }
        | Instruction::IputShort { .. }
        | Instruction::Sput { .. }
        | Instruction::SputWide { .. }
        | Instruction::SputObject { .. }
        | Instruction::SputBoolean { .. }
        | Instruction::SputByte { .. }
        | Instruction::SputChar { .. }
        | Instruction::SputShort { .. }
        | Instruction::FilledNewArray { .. }
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
        | Instruction::InvokeCustomRange { .. }
        | Instruction::PackedSwitchPayload { .. }
        | Instruction::SparseSwitchPayload { .. }
        | Instruction::FillArrayDataPayload { .. }
        | Instruction::RawInstruction { .. } => vec![],
    }
}

fn read_registers(insn: &Instruction) -> Vec<u16> {
    match insn {
        Instruction::Move { src, .. }
        | Instruction::MoveWide { src, .. }
        | Instruction::MoveObject { src, .. } => vec![*src as u16],

        Instruction::MoveFrom16 { src, .. }
        | Instruction::MoveWideFrom16 { src, .. }
        | Instruction::MoveObjectFrom16 { src, .. } => vec![*src],

        Instruction::Move16 { src, .. }
        | Instruction::MoveWide16 { src, .. }
        | Instruction::MoveObject16 { src, .. } => vec![*src],

        Instruction::Return { src }
        | Instruction::ReturnWide { src }
        | Instruction::ReturnObject { src } => vec![*src as u16],

        Instruction::MonitorEnter { ref_ } | Instruction::MonitorExit { ref_ } => {
            vec![*ref_ as u16]
        }

        Instruction::CheckCast { ref_, .. } => vec![*ref_ as u16],
        Instruction::InstanceOf { ref_, .. } => vec![*ref_ as u16],
        Instruction::ArrayLength { array, .. } => vec![*array as u16],
        Instruction::NewArray { size, .. } => vec![*size as u16],
        Instruction::Throw { exception } => vec![*exception as u16],

        Instruction::FillArrayData { array, .. } => vec![*array as u16],
        Instruction::PackedSwitch { test, .. } | Instruction::SparseSwitch { test, .. } => {
            vec![*test as u16]
        }

        Instruction::IfEq { a, b, .. }
        | Instruction::IfNe { a, b, .. }
        | Instruction::IfLt { a, b, .. }
        | Instruction::IfGe { a, b, .. }
        | Instruction::IfGt { a, b, .. }
        | Instruction::IfLe { a, b, .. } => vec![*a as u16, *b as u16],

        Instruction::IfEqz { a, .. }
        | Instruction::IfNez { a, .. }
        | Instruction::IfLtz { a, .. }
        | Instruction::IfGez { a, .. }
        | Instruction::IfGtz { a, .. }
        | Instruction::IfLez { a, .. } => vec![*a as u16],

        Instruction::Aget { array, index, .. }
        | Instruction::AgetWide { array, index, .. }
        | Instruction::AgetObject { array, index, .. }
        | Instruction::AgetBoolean { array, index, .. }
        | Instruction::AgetByte { array, index, .. }
        | Instruction::AgetChar { array, index, .. }
        | Instruction::AgetShort { array, index, .. } => vec![*array as u16, *index as u16],

        Instruction::Aput {
            src, array, index, ..
        }
        | Instruction::AputWide {
            src, array, index, ..
        }
        | Instruction::AputObject {
            src, array, index, ..
        }
        | Instruction::AputBoolean {
            src, array, index, ..
        }
        | Instruction::AputByte {
            src, array, index, ..
        }
        | Instruction::AputChar {
            src, array, index, ..
        }
        | Instruction::AputShort {
            src, array, index, ..
        } => vec![*src as u16, *array as u16, *index as u16],

        Instruction::Iget { obj, .. }
        | Instruction::IgetWide { obj, .. }
        | Instruction::IgetObject { obj, .. }
        | Instruction::IgetBoolean { obj, .. }
        | Instruction::IgetByte { obj, .. }
        | Instruction::IgetChar { obj, .. }
        | Instruction::IgetShort { obj, .. } => vec![*obj as u16],

        Instruction::Iput { src, obj, .. }
        | Instruction::IputWide { src, obj, .. }
        | Instruction::IputObject { src, obj, .. }
        | Instruction::IputBoolean { src, obj, .. }
        | Instruction::IputByte { src, obj, .. }
        | Instruction::IputChar { src, obj, .. }
        | Instruction::IputShort { src, obj, .. } => vec![*src as u16, *obj as u16],

        Instruction::Sput { src, .. }
        | Instruction::SputWide { src, .. }
        | Instruction::SputObject { src, .. }
        | Instruction::SputBoolean { src, .. }
        | Instruction::SputByte { src, .. }
        | Instruction::SputChar { src, .. }
        | Instruction::SputShort { src, .. } => vec![*src as u16],

        Instruction::CmpLFloat { a, b, .. }
        | Instruction::CmpGFloat { a, b, .. }
        | Instruction::CmpLDouble { a, b, .. }
        | Instruction::CmpGDouble { a, b, .. }
        | Instruction::CmpLong { a, b, .. }
        | Instruction::AddInt { a, b, .. }
        | Instruction::SubInt { a, b, .. }
        | Instruction::MulInt { a, b, .. }
        | Instruction::DivInt { a, b, .. }
        | Instruction::RemInt { a, b, .. }
        | Instruction::AndInt { a, b, .. }
        | Instruction::OrInt { a, b, .. }
        | Instruction::XorInt { a, b, .. }
        | Instruction::ShlInt { a, b, .. }
        | Instruction::ShrInt { a, b, .. }
        | Instruction::UshrInt { a, b, .. }
        | Instruction::AddLong { a, b, .. }
        | Instruction::SubLong { a, b, .. }
        | Instruction::MulLong { a, b, .. }
        | Instruction::DivLong { a, b, .. }
        | Instruction::RemLong { a, b, .. }
        | Instruction::AndLong { a, b, .. }
        | Instruction::OrLong { a, b, .. }
        | Instruction::XorLong { a, b, .. }
        | Instruction::ShlLong { a, b, .. }
        | Instruction::ShrLong { a, b, .. }
        | Instruction::UshrLong { a, b, .. }
        | Instruction::AddFloat { a, b, .. }
        | Instruction::SubFloat { a, b, .. }
        | Instruction::MulFloat { a, b, .. }
        | Instruction::DivFloat { a, b, .. }
        | Instruction::RemFloat { a, b, .. }
        | Instruction::AddDouble { a, b, .. }
        | Instruction::SubDouble { a, b, .. }
        | Instruction::MulDouble { a, b, .. }
        | Instruction::DivDouble { a, b, .. }
        | Instruction::RemDouble { a, b, .. } => vec![*a as u16, *b as u16],

        Instruction::AddInt2Addr { dest_a, b }
        | Instruction::SubInt2Addr { dest_a, b }
        | Instruction::MulInt2Addr { dest_a, b }
        | Instruction::DivInt2Addr { dest_a, b }
        | Instruction::RemInt2Addr { dest_a, b }
        | Instruction::AndInt2Addr { dest_a, b }
        | Instruction::OrInt2Addr { dest_a, b }
        | Instruction::XorInt2Addr { dest_a, b }
        | Instruction::ShlInt2Addr { dest_a, b }
        | Instruction::ShrInt2Addr { dest_a, b }
        | Instruction::UshrInt2Addr { dest_a, b }
        | Instruction::AddLong2Addr { dest_a, b }
        | Instruction::SubLong2Addr { dest_a, b }
        | Instruction::MulLong2Addr { dest_a, b }
        | Instruction::DivLong2Addr { dest_a, b }
        | Instruction::RemLong2Addr { dest_a, b }
        | Instruction::AndLong2Addr { dest_a, b }
        | Instruction::OrLong2Addr { dest_a, b }
        | Instruction::XorLong2Addr { dest_a, b }
        | Instruction::ShlLong2Addr { dest_a, b }
        | Instruction::ShrLong2Addr { dest_a, b }
        | Instruction::UshrLong2Addr { dest_a, b }
        | Instruction::AddFloat2Addr { dest_a, b }
        | Instruction::SubFloat2Addr { dest_a, b }
        | Instruction::MulFloat2Addr { dest_a, b }
        | Instruction::DivFloat2Addr { dest_a, b }
        | Instruction::RemFloat2Addr { dest_a, b }
        | Instruction::AddDouble2Addr { dest_a, b }
        | Instruction::SubDouble2Addr { dest_a, b }
        | Instruction::MulDouble2Addr { dest_a, b }
        | Instruction::DivDouble2Addr { dest_a, b }
        | Instruction::RemDouble2Addr { dest_a, b } => vec![*dest_a as u16, *b as u16],

        Instruction::NegInt { src, .. }
        | Instruction::NotInt { src, .. }
        | Instruction::NegLong { src, .. }
        | Instruction::NotLong { src, .. }
        | Instruction::NegFloat { src, .. }
        | Instruction::NegDouble { src, .. }
        | Instruction::IntToLong { src, .. }
        | Instruction::IntToFloat { src, .. }
        | Instruction::IntToDouble { src, .. }
        | Instruction::LongToInt { src, .. }
        | Instruction::LongToFloat { src, .. }
        | Instruction::LongToDouble { src, .. }
        | Instruction::FloatToInt { src, .. }
        | Instruction::FloatToLong { src, .. }
        | Instruction::FloatToDouble { src, .. }
        | Instruction::DoubleToInt { src, .. }
        | Instruction::DoubleToLong { src, .. }
        | Instruction::DoubleToFloat { src, .. }
        | Instruction::IntToByte { src, .. }
        | Instruction::IntToChar { src, .. }
        | Instruction::IntToShort { src, .. } => vec![*src as u16],

        Instruction::AddIntLit16 { src, .. }
        | Instruction::RsubIntLit16 { src, .. }
        | Instruction::MulIntLit16 { src, .. }
        | Instruction::DivIntLit16 { src, .. }
        | Instruction::RemIntLit16 { src, .. }
        | Instruction::AndIntLit16 { src, .. }
        | Instruction::OrIntLit16 { src, .. }
        | Instruction::XorIntLit16 { src, .. } => vec![*src as u16],

        Instruction::AddIntLit8 { src, .. }
        | Instruction::RsubIntLit8 { src, .. }
        | Instruction::MulIntLit8 { src, .. }
        | Instruction::DivIntLit8 { src, .. }
        | Instruction::RemIntLit8 { src, .. }
        | Instruction::AndIntLit8 { src, .. }
        | Instruction::OrIntLit8 { src, .. }
        | Instruction::XorIntLit8 { src, .. }
        | Instruction::ShlIntLit8 { src, .. }
        | Instruction::ShrIntLit8 { src, .. }
        | Instruction::UshrIntLit8 { src, .. } => vec![*src as u16],

        Instruction::FilledNewArray { args, .. } | Instruction::InvokeCustom { args, .. } => {
            args.iter().map(|r| *r as u16).collect()
        }

        Instruction::InvokeVirtual { args, .. }
        | Instruction::InvokeSuper { args, .. }
        | Instruction::InvokeDirect { args, .. }
        | Instruction::InvokeStatic { args, .. }
        | Instruction::InvokeInterface { args, .. } => args.iter().map(|r| *r as u16).collect(),

        Instruction::InvokePolymorphic { args, .. } => args.iter().map(|r| *r as u16).collect(),

        Instruction::FilledNewArrayRange {
            first_reg, count, ..
        }
        | Instruction::InvokeVirtualRange {
            first_reg, count, ..
        }
        | Instruction::InvokeSuperRange {
            first_reg, count, ..
        }
        | Instruction::InvokeDirectRange {
            first_reg, count, ..
        }
        | Instruction::InvokeStaticRange {
            first_reg, count, ..
        }
        | Instruction::InvokeInterfaceRange {
            first_reg, count, ..
        }
        | Instruction::InvokeCustomRange {
            first_reg, count, ..
        } => (*first_reg..first_reg + *count as u16).collect(),

        Instruction::InvokePolymorphicRange {
            first_reg, count, ..
        } => (*first_reg..first_reg + *count as u16).collect(),

        Instruction::Nop
        | Instruction::ReturnVoid
        | Instruction::MoveResult { .. }
        | Instruction::MoveResultWide { .. }
        | Instruction::MoveResultObject { .. }
        | Instruction::MoveException { .. }
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
        | Instruction::NewInstance { .. }
        | Instruction::ConstMethodHandle { .. }
        | Instruction::ConstMethodType { .. }
        | Instruction::Sget { .. }
        | Instruction::SgetWide { .. }
        | Instruction::SgetObject { .. }
        | Instruction::SgetBoolean { .. }
        | Instruction::SgetByte { .. }
        | Instruction::SgetChar { .. }
        | Instruction::SgetShort { .. }
        | Instruction::Goto { .. }
        | Instruction::Goto16 { .. }
        | Instruction::Goto32 { .. }
        | Instruction::PackedSwitchPayload { .. }
        | Instruction::SparseSwitchPayload { .. }
        | Instruction::FillArrayDataPayload { .. }
        | Instruction::RawInstruction { .. } => vec![],
    }
}
