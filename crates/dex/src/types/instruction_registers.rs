// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::instruction::Instruction;

impl Instruction {
    /// Calls `visit` for each register read by this instruction.
    pub fn visit_read_registers(&self, mut visit: impl FnMut(u16)) {
        match self {
            Instruction::Move { src, .. }
            | Instruction::MoveWide { src, .. }
            | Instruction::MoveObject { src, .. } => visit(u16::from(*src)),

            Instruction::MoveFrom16 { src, .. }
            | Instruction::MoveWideFrom16 { src, .. }
            | Instruction::MoveObjectFrom16 { src, .. }
            | Instruction::Move16 { src, .. }
            | Instruction::MoveWide16 { src, .. }
            | Instruction::MoveObject16 { src, .. } => visit(*src),

            Instruction::Return { src }
            | Instruction::ReturnWide { src }
            | Instruction::ReturnObject { src }
            | Instruction::MonitorEnter { ref_: src }
            | Instruction::MonitorExit { ref_: src }
            | Instruction::CheckCast { ref_: src, .. }
            | Instruction::InstanceOf { ref_: src, .. }
            | Instruction::Throw { exception: src }
            | Instruction::FillArrayData { array: src, .. }
            | Instruction::PackedSwitch { test: src, .. }
            | Instruction::SparseSwitch { test: src, .. }
            | Instruction::AddIntLit16 { src, .. }
            | Instruction::RsubIntLit16 { src, .. }
            | Instruction::MulIntLit16 { src, .. }
            | Instruction::DivIntLit16 { src, .. }
            | Instruction::RemIntLit16 { src, .. }
            | Instruction::AndIntLit16 { src, .. }
            | Instruction::OrIntLit16 { src, .. }
            | Instruction::XorIntLit16 { src, .. }
            | Instruction::AddIntLit8 { src, .. }
            | Instruction::RsubIntLit8 { src, .. }
            | Instruction::MulIntLit8 { src, .. }
            | Instruction::DivIntLit8 { src, .. }
            | Instruction::RemIntLit8 { src, .. }
            | Instruction::AndIntLit8 { src, .. }
            | Instruction::OrIntLit8 { src, .. }
            | Instruction::XorIntLit8 { src, .. }
            | Instruction::ShlIntLit8 { src, .. }
            | Instruction::ShrIntLit8 { src, .. }
            | Instruction::UshrIntLit8 { src, .. } => visit(u16::from(*src)),

            Instruction::ArrayLength { array, .. } | Instruction::NewArray { size: array, .. } => {
                visit(u16::from(*array))
            }

            Instruction::IfEq { a, b, .. }
            | Instruction::IfNe { a, b, .. }
            | Instruction::IfLt { a, b, .. }
            | Instruction::IfGe { a, b, .. }
            | Instruction::IfGt { a, b, .. }
            | Instruction::IfLe { a, b, .. }
            | Instruction::CmpLFloat { a, b, .. }
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
            | Instruction::RemDouble { a, b, .. } => {
                visit(u16::from(*a));
                visit(u16::from(*b));
            }

            Instruction::IfEqz { a, .. }
            | Instruction::IfNez { a, .. }
            | Instruction::IfLtz { a, .. }
            | Instruction::IfGez { a, .. }
            | Instruction::IfGtz { a, .. }
            | Instruction::IfLez { a, .. }
            | Instruction::NegInt { src: a, .. }
            | Instruction::NotInt { src: a, .. }
            | Instruction::NegLong { src: a, .. }
            | Instruction::NotLong { src: a, .. }
            | Instruction::NegFloat { src: a, .. }
            | Instruction::NegDouble { src: a, .. }
            | Instruction::IntToLong { src: a, .. }
            | Instruction::IntToFloat { src: a, .. }
            | Instruction::IntToDouble { src: a, .. }
            | Instruction::LongToInt { src: a, .. }
            | Instruction::LongToFloat { src: a, .. }
            | Instruction::LongToDouble { src: a, .. }
            | Instruction::FloatToInt { src: a, .. }
            | Instruction::FloatToLong { src: a, .. }
            | Instruction::FloatToDouble { src: a, .. }
            | Instruction::DoubleToInt { src: a, .. }
            | Instruction::DoubleToLong { src: a, .. }
            | Instruction::DoubleToFloat { src: a, .. }
            | Instruction::IntToByte { src: a, .. }
            | Instruction::IntToChar { src: a, .. }
            | Instruction::IntToShort { src: a, .. } => visit(u16::from(*a)),

            Instruction::Aget { array, index, .. }
            | Instruction::AgetWide { array, index, .. }
            | Instruction::AgetObject { array, index, .. }
            | Instruction::AgetBoolean { array, index, .. }
            | Instruction::AgetByte { array, index, .. }
            | Instruction::AgetChar { array, index, .. }
            | Instruction::AgetShort { array, index, .. } => {
                visit(u16::from(*array));
                visit(u16::from(*index));
            }

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
            } => {
                visit(u16::from(*src));
                visit(u16::from(*array));
                visit(u16::from(*index));
            }

            Instruction::Iget { obj, .. }
            | Instruction::IgetWide { obj, .. }
            | Instruction::IgetObject { obj, .. }
            | Instruction::IgetBoolean { obj, .. }
            | Instruction::IgetByte { obj, .. }
            | Instruction::IgetChar { obj, .. }
            | Instruction::IgetShort { obj, .. } => visit(u16::from(*obj)),

            Instruction::Iput { src, obj, .. }
            | Instruction::IputWide { src, obj, .. }
            | Instruction::IputObject { src, obj, .. }
            | Instruction::IputBoolean { src, obj, .. }
            | Instruction::IputByte { src, obj, .. }
            | Instruction::IputChar { src, obj, .. }
            | Instruction::IputShort { src, obj, .. } => {
                visit(u16::from(*src));
                visit(u16::from(*obj));
            }

            Instruction::Sput { src, .. }
            | Instruction::SputWide { src, .. }
            | Instruction::SputObject { src, .. }
            | Instruction::SputBoolean { src, .. }
            | Instruction::SputByte { src, .. }
            | Instruction::SputChar { src, .. }
            | Instruction::SputShort { src, .. } => visit(u16::from(*src)),

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
            | Instruction::RemDouble2Addr { dest_a, b } => {
                visit(u16::from(*dest_a));
                visit(u16::from(*b));
            }

            Instruction::FilledNewArray { args, .. }
            | Instruction::InvokeVirtual { args, .. }
            | Instruction::InvokeSuper { args, .. }
            | Instruction::InvokeDirect { args, .. }
            | Instruction::InvokeStatic { args, .. }
            | Instruction::InvokeInterface { args, .. }
            | Instruction::InvokePolymorphic { args, .. }
            | Instruction::InvokeCustom { args, .. } => {
                for &register in args {
                    visit(u16::from(register));
                }
            }

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
            | Instruction::InvokePolymorphicRange {
                first_reg, count, ..
            }
            | Instruction::InvokeCustomRange {
                first_reg, count, ..
            } => {
                for register in *first_reg..*first_reg + u16::from(*count) {
                    visit(register);
                }
            }

            Instruction::Nop
            | Instruction::MoveResult { .. }
            | Instruction::MoveResultWide { .. }
            | Instruction::MoveResultObject { .. }
            | Instruction::MoveException { .. }
            | Instruction::ReturnVoid
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
            | Instruction::RawInstruction { .. } => {}
        }
    }

    /// Calls `visit` for each register written by this instruction.
    pub fn visit_written_registers(&self, mut visit: impl FnMut(u16)) {
        match self {
            Instruction::Move { dest, .. }
            | Instruction::MoveWide { dest, .. }
            | Instruction::MoveObject { dest, .. }
            | Instruction::Const4 { dest, .. }
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
            | Instruction::AddInt2Addr { dest_a: dest, .. }
            | Instruction::SubInt2Addr { dest_a: dest, .. }
            | Instruction::MulInt2Addr { dest_a: dest, .. }
            | Instruction::DivInt2Addr { dest_a: dest, .. }
            | Instruction::RemInt2Addr { dest_a: dest, .. }
            | Instruction::AndInt2Addr { dest_a: dest, .. }
            | Instruction::OrInt2Addr { dest_a: dest, .. }
            | Instruction::XorInt2Addr { dest_a: dest, .. }
            | Instruction::ShlInt2Addr { dest_a: dest, .. }
            | Instruction::ShrInt2Addr { dest_a: dest, .. }
            | Instruction::UshrInt2Addr { dest_a: dest, .. }
            | Instruction::AddLong2Addr { dest_a: dest, .. }
            | Instruction::SubLong2Addr { dest_a: dest, .. }
            | Instruction::MulLong2Addr { dest_a: dest, .. }
            | Instruction::DivLong2Addr { dest_a: dest, .. }
            | Instruction::RemLong2Addr { dest_a: dest, .. }
            | Instruction::AndLong2Addr { dest_a: dest, .. }
            | Instruction::OrLong2Addr { dest_a: dest, .. }
            | Instruction::XorLong2Addr { dest_a: dest, .. }
            | Instruction::ShlLong2Addr { dest_a: dest, .. }
            | Instruction::ShrLong2Addr { dest_a: dest, .. }
            | Instruction::UshrLong2Addr { dest_a: dest, .. }
            | Instruction::AddFloat2Addr { dest_a: dest, .. }
            | Instruction::SubFloat2Addr { dest_a: dest, .. }
            | Instruction::MulFloat2Addr { dest_a: dest, .. }
            | Instruction::DivFloat2Addr { dest_a: dest, .. }
            | Instruction::RemFloat2Addr { dest_a: dest, .. }
            | Instruction::AddDouble2Addr { dest_a: dest, .. }
            | Instruction::SubDouble2Addr { dest_a: dest, .. }
            | Instruction::MulDouble2Addr { dest_a: dest, .. }
            | Instruction::DivDouble2Addr { dest_a: dest, .. }
            | Instruction::RemDouble2Addr { dest_a: dest, .. } => visit(u16::from(*dest)),

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
            | Instruction::UshrIntLit8 { dest, .. }
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
            | Instruction::XorIntLit16 { dest, .. } => visit(u16::from(*dest)),

            Instruction::Move16 { dest, .. }
            | Instruction::MoveWide16 { dest, .. }
            | Instruction::MoveObject16 { dest, .. } => visit(*dest),

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
            | Instruction::RawInstruction { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use smallvec::smallvec;

    use super::Instruction;
    use crate::types::{MethodIdx, TypeIdx};

    fn collected_reads(insn: &Instruction) -> Vec<u16> {
        let mut out = Vec::new();
        insn.visit_read_registers(|register| out.push(register));
        out
    }

    fn collected_writes(insn: &Instruction) -> Vec<u16> {
        let mut out = Vec::new();
        insn.visit_written_registers(|register| out.push(register));
        out
    }

    #[test]
    fn collects_binary_op_registers() {
        let insn = Instruction::AddInt {
            dest: 1,
            a: 2,
            b: 3,
        };

        assert_eq!(collected_reads(&insn), vec![2, 3]);
        assert_eq!(collected_writes(&insn), vec![1]);
    }

    #[test]
    fn collects_invoke_arg_registers() {
        let insn = Instruction::InvokeStatic {
            method: MethodIdx(0),
            args: smallvec![1, 4, 7],
        };

        assert_eq!(collected_reads(&insn), vec![1, 4, 7]);
        assert!(collected_writes(&insn).is_empty());
    }

    #[test]
    fn collects_invoke_range_registers() {
        let insn = Instruction::FilledNewArrayRange {
            type_: TypeIdx(0),
            first_reg: 5,
            count: 3,
        };

        assert_eq!(collected_reads(&insn), vec![5, 6, 7]);
        assert!(collected_writes(&insn).is_empty());
    }
}
