// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use smallvec::SmallVec;

use super::instruction::Instruction;

impl Instruction {
    pub fn dest_register(&self) -> Option<u16> {
        match self {
            Self::Move { dest, .. }
            | Self::MoveWide { dest, .. }
            | Self::MoveObject { dest, .. }
            | Self::Const4 { dest, .. }
            | Self::InstanceOf { dest, .. }
            | Self::ArrayLength { dest, .. }
            | Self::NewArray { dest, .. }
            | Self::NegInt { dest, .. }
            | Self::NotInt { dest, .. }
            | Self::NegLong { dest, .. }
            | Self::NotLong { dest, .. }
            | Self::NegFloat { dest, .. }
            | Self::NegDouble { dest, .. }
            | Self::IntToLong { dest, .. }
            | Self::IntToFloat { dest, .. }
            | Self::IntToDouble { dest, .. }
            | Self::LongToInt { dest, .. }
            | Self::LongToFloat { dest, .. }
            | Self::LongToDouble { dest, .. }
            | Self::FloatToInt { dest, .. }
            | Self::FloatToLong { dest, .. }
            | Self::FloatToDouble { dest, .. }
            | Self::DoubleToInt { dest, .. }
            | Self::DoubleToLong { dest, .. }
            | Self::DoubleToFloat { dest, .. }
            | Self::IntToByte { dest, .. }
            | Self::IntToChar { dest, .. }
            | Self::IntToShort { dest, .. }
            | Self::AddIntLit16 { dest, .. }
            | Self::RsubIntLit16 { dest, .. }
            | Self::MulIntLit16 { dest, .. }
            | Self::DivIntLit16 { dest, .. }
            | Self::RemIntLit16 { dest, .. }
            | Self::AndIntLit16 { dest, .. }
            | Self::OrIntLit16 { dest, .. }
            | Self::XorIntLit16 { dest, .. } => Some(u16::from(*dest)),
            Self::Iget { dest, .. }
            | Self::IgetWide { dest, .. }
            | Self::IgetObject { dest, .. }
            | Self::IgetBoolean { dest, .. }
            | Self::IgetByte { dest, .. }
            | Self::IgetChar { dest, .. }
            | Self::IgetShort { dest, .. } => Some(u16::from(*dest)),
            Self::MoveFrom16 { dest, .. }
            | Self::MoveWideFrom16 { dest, .. }
            | Self::MoveObjectFrom16 { dest, .. }
            | Self::MoveResult { dest, .. }
            | Self::MoveResultWide { dest, .. }
            | Self::MoveResultObject { dest, .. }
            | Self::MoveException { dest, .. }
            | Self::Const16 { dest, .. }
            | Self::Const { dest, .. }
            | Self::ConstHigh16 { dest, .. }
            | Self::ConstWide16 { dest, .. }
            | Self::ConstWide32 { dest, .. }
            | Self::ConstWide { dest, .. }
            | Self::ConstWideHigh16 { dest, .. }
            | Self::ConstString { dest, .. }
            | Self::ConstStringJumbo { dest, .. }
            | Self::ConstClass { dest, .. }
            | Self::NewInstance { dest, .. }
            | Self::ConstMethodHandle { dest, .. }
            | Self::ConstMethodType { dest, .. }
            | Self::CmpLFloat { dest, .. }
            | Self::CmpGFloat { dest, .. }
            | Self::CmpLDouble { dest, .. }
            | Self::CmpGDouble { dest, .. }
            | Self::CmpLong { dest, .. }
            | Self::Sget { dest, .. }
            | Self::SgetWide { dest, .. }
            | Self::SgetObject { dest, .. }
            | Self::SgetBoolean { dest, .. }
            | Self::SgetByte { dest, .. }
            | Self::SgetChar { dest, .. }
            | Self::SgetShort { dest, .. }
            | Self::Aget { dest, .. }
            | Self::AgetWide { dest, .. }
            | Self::AgetObject { dest, .. }
            | Self::AgetBoolean { dest, .. }
            | Self::AgetByte { dest, .. }
            | Self::AgetChar { dest, .. }
            | Self::AgetShort { dest, .. }
            | Self::AddInt { dest, .. }
            | Self::SubInt { dest, .. }
            | Self::MulInt { dest, .. }
            | Self::DivInt { dest, .. }
            | Self::RemInt { dest, .. }
            | Self::AndInt { dest, .. }
            | Self::OrInt { dest, .. }
            | Self::XorInt { dest, .. }
            | Self::ShlInt { dest, .. }
            | Self::ShrInt { dest, .. }
            | Self::UshrInt { dest, .. }
            | Self::AddLong { dest, .. }
            | Self::SubLong { dest, .. }
            | Self::MulLong { dest, .. }
            | Self::DivLong { dest, .. }
            | Self::RemLong { dest, .. }
            | Self::AndLong { dest, .. }
            | Self::OrLong { dest, .. }
            | Self::XorLong { dest, .. }
            | Self::ShlLong { dest, .. }
            | Self::ShrLong { dest, .. }
            | Self::UshrLong { dest, .. }
            | Self::AddFloat { dest, .. }
            | Self::SubFloat { dest, .. }
            | Self::MulFloat { dest, .. }
            | Self::DivFloat { dest, .. }
            | Self::RemFloat { dest, .. }
            | Self::AddDouble { dest, .. }
            | Self::SubDouble { dest, .. }
            | Self::MulDouble { dest, .. }
            | Self::DivDouble { dest, .. }
            | Self::RemDouble { dest, .. }
            | Self::AddIntLit8 { dest, .. }
            | Self::RsubIntLit8 { dest, .. }
            | Self::MulIntLit8 { dest, .. }
            | Self::DivIntLit8 { dest, .. }
            | Self::RemIntLit8 { dest, .. }
            | Self::AndIntLit8 { dest, .. }
            | Self::OrIntLit8 { dest, .. }
            | Self::XorIntLit8 { dest, .. }
            | Self::ShlIntLit8 { dest, .. }
            | Self::ShrIntLit8 { dest, .. }
            | Self::UshrIntLit8 { dest, .. } => Some(u16::from(*dest)),
            Self::Move16 { dest, .. }
            | Self::MoveWide16 { dest, .. }
            | Self::MoveObject16 { dest, .. } => Some(*dest),
            Self::AddInt2Addr { dest_a, .. }
            | Self::SubInt2Addr { dest_a, .. }
            | Self::MulInt2Addr { dest_a, .. }
            | Self::DivInt2Addr { dest_a, .. }
            | Self::RemInt2Addr { dest_a, .. }
            | Self::AndInt2Addr { dest_a, .. }
            | Self::OrInt2Addr { dest_a, .. }
            | Self::XorInt2Addr { dest_a, .. }
            | Self::ShlInt2Addr { dest_a, .. }
            | Self::ShrInt2Addr { dest_a, .. }
            | Self::UshrInt2Addr { dest_a, .. }
            | Self::AddLong2Addr { dest_a, .. }
            | Self::SubLong2Addr { dest_a, .. }
            | Self::MulLong2Addr { dest_a, .. }
            | Self::DivLong2Addr { dest_a, .. }
            | Self::RemLong2Addr { dest_a, .. }
            | Self::AndLong2Addr { dest_a, .. }
            | Self::OrLong2Addr { dest_a, .. }
            | Self::XorLong2Addr { dest_a, .. }
            | Self::ShlLong2Addr { dest_a, .. }
            | Self::ShrLong2Addr { dest_a, .. }
            | Self::UshrLong2Addr { dest_a, .. }
            | Self::AddFloat2Addr { dest_a, .. }
            | Self::SubFloat2Addr { dest_a, .. }
            | Self::MulFloat2Addr { dest_a, .. }
            | Self::DivFloat2Addr { dest_a, .. }
            | Self::RemFloat2Addr { dest_a, .. }
            | Self::AddDouble2Addr { dest_a, .. }
            | Self::SubDouble2Addr { dest_a, .. }
            | Self::MulDouble2Addr { dest_a, .. }
            | Self::DivDouble2Addr { dest_a, .. }
            | Self::RemDouble2Addr { dest_a, .. } => Some(u16::from(*dest_a)),
            Self::CheckCast { ref_, .. } => Some(u16::from(*ref_)),
            _ => None,
        }
    }

    pub fn write_register(&self) -> Option<u16> {
        match self {
            Self::Return { .. }
            | Self::ReturnWide { .. }
            | Self::ReturnObject { .. }
            | Self::ReturnVoid
            | Self::Goto { .. }
            | Self::Goto16 { .. }
            | Self::Goto32 { .. }
            | Self::IfEq { .. }
            | Self::IfNe { .. }
            | Self::IfLt { .. }
            | Self::IfGe { .. }
            | Self::IfGt { .. }
            | Self::IfLe { .. }
            | Self::IfEqz { .. }
            | Self::IfNez { .. }
            | Self::IfLtz { .. }
            | Self::IfGez { .. }
            | Self::IfGtz { .. }
            | Self::IfLez { .. }
            | Self::Throw { .. }
            | Self::MonitorEnter { .. }
            | Self::MonitorExit { .. }
            | Self::PackedSwitch { .. }
            | Self::SparseSwitch { .. }
            | Self::Iput { .. }
            | Self::IputWide { .. }
            | Self::IputObject { .. }
            | Self::IputBoolean { .. }
            | Self::IputByte { .. }
            | Self::IputChar { .. }
            | Self::IputShort { .. }
            | Self::Sput { .. }
            | Self::SputWide { .. }
            | Self::SputObject { .. }
            | Self::SputBoolean { .. }
            | Self::SputByte { .. }
            | Self::SputChar { .. }
            | Self::SputShort { .. }
            | Self::Aput { .. }
            | Self::AputWide { .. }
            | Self::AputObject { .. }
            | Self::AputBoolean { .. }
            | Self::AputByte { .. }
            | Self::AputChar { .. }
            | Self::AputShort { .. }
            | Self::InvokeVirtual { .. }
            | Self::InvokeSuper { .. }
            | Self::InvokeDirect { .. }
            | Self::InvokeStatic { .. }
            | Self::InvokeInterface { .. }
            | Self::InvokeVirtualRange { .. }
            | Self::InvokeSuperRange { .. }
            | Self::InvokeDirectRange { .. }
            | Self::InvokeStaticRange { .. }
            | Self::InvokeInterfaceRange { .. }
            | Self::InvokePolymorphic { .. }
            | Self::InvokePolymorphicRange { .. }
            | Self::InvokeCustom { .. }
            | Self::InvokeCustomRange { .. }
            | Self::FillArrayData { .. }
            | Self::FilledNewArray { .. }
            | Self::FilledNewArrayRange { .. }
            | Self::Nop
            | Self::PackedSwitchPayload { .. }
            | Self::SparseSwitchPayload { .. }
            | Self::FillArrayDataPayload { .. }
            | Self::RawInstruction { .. } => None,
            other => other.dest_register(),
        }
    }

    pub fn registers_used(&self) -> SmallVec<[u16; 6]> {
        let mut regs = SmallVec::new();
        match self {
            Self::Nop
            | Self::ReturnVoid
            | Self::Goto { .. }
            | Self::Goto16 { .. }
            | Self::Goto32 { .. }
            | Self::PackedSwitchPayload { .. }
            | Self::SparseSwitchPayload { .. }
            | Self::FillArrayDataPayload { .. }
            | Self::RawInstruction { .. } => {}
            Self::Move { dest, src }
            | Self::MoveWide { dest, src }
            | Self::MoveObject { dest, src } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*src));
            }
            Self::MoveFrom16 { dest, src }
            | Self::MoveWideFrom16 { dest, src }
            | Self::MoveObjectFrom16 { dest, src } => {
                regs.push(u16::from(*dest));
                regs.push(*src);
            }
            Self::Move16 { dest, src }
            | Self::MoveWide16 { dest, src }
            | Self::MoveObject16 { dest, src } => {
                regs.push(*dest);
                regs.push(*src);
            }
            Self::MoveResult { dest }
            | Self::MoveResultWide { dest }
            | Self::MoveResultObject { dest }
            | Self::MoveException { dest } => regs.push(u16::from(*dest)),
            Self::Return { src } | Self::ReturnWide { src } | Self::ReturnObject { src } => {
                regs.push(u16::from(*src));
            }
            Self::Const4 { dest, .. } => regs.push(u16::from(*dest)),
            Self::Const16 { dest, .. }
            | Self::Const { dest, .. }
            | Self::ConstHigh16 { dest, .. }
            | Self::ConstWide16 { dest, .. }
            | Self::ConstWide32 { dest, .. }
            | Self::ConstWide { dest, .. }
            | Self::ConstWideHigh16 { dest, .. }
            | Self::ConstString { dest, .. }
            | Self::ConstStringJumbo { dest, .. }
            | Self::ConstClass { dest, .. }
            | Self::NewInstance { dest, .. }
            | Self::ConstMethodHandle { dest, .. }
            | Self::ConstMethodType { dest, .. } => regs.push(u16::from(*dest)),
            Self::MonitorEnter { ref_ } | Self::MonitorExit { ref_ } => {
                regs.push(u16::from(*ref_));
            }
            Self::CheckCast { ref_, .. } => regs.push(u16::from(*ref_)),
            Self::InstanceOf { dest, ref_, .. } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*ref_));
            }
            Self::ArrayLength { dest, array } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*array));
            }
            Self::NewArray { dest, size, .. } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*size));
            }
            Self::FilledNewArray { args, .. } => {
                for register in args {
                    regs.push(u16::from(*register));
                }
            }
            Self::FilledNewArrayRange {
                first_reg, count, ..
            } => {
                for offset in 0..u16::from(*count) {
                    regs.push(*first_reg + offset);
                }
            }
            Self::FillArrayData { array, .. } => regs.push(u16::from(*array)),
            Self::Throw { exception } => regs.push(u16::from(*exception)),
            Self::PackedSwitch { test, .. } | Self::SparseSwitch { test, .. } => {
                regs.push(u16::from(*test));
            }
            Self::CmpLFloat { dest, a, b }
            | Self::CmpGFloat { dest, a, b }
            | Self::CmpLDouble { dest, a, b }
            | Self::CmpGDouble { dest, a, b }
            | Self::CmpLong { dest, a, b } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*a));
                regs.push(u16::from(*b));
            }
            Self::IfEq { a, b, .. }
            | Self::IfNe { a, b, .. }
            | Self::IfLt { a, b, .. }
            | Self::IfGe { a, b, .. }
            | Self::IfGt { a, b, .. }
            | Self::IfLe { a, b, .. } => {
                regs.push(u16::from(*a));
                regs.push(u16::from(*b));
            }
            Self::IfEqz { a, .. }
            | Self::IfNez { a, .. }
            | Self::IfLtz { a, .. }
            | Self::IfGez { a, .. }
            | Self::IfGtz { a, .. }
            | Self::IfLez { a, .. } => regs.push(u16::from(*a)),
            Self::Aget { dest, array, index }
            | Self::AgetWide { dest, array, index }
            | Self::AgetObject { dest, array, index }
            | Self::AgetBoolean { dest, array, index }
            | Self::AgetByte { dest, array, index }
            | Self::AgetChar { dest, array, index }
            | Self::AgetShort { dest, array, index } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*array));
                regs.push(u16::from(*index));
            }
            Self::Aput { src, array, index }
            | Self::AputWide { src, array, index }
            | Self::AputObject { src, array, index }
            | Self::AputBoolean { src, array, index }
            | Self::AputByte { src, array, index }
            | Self::AputChar { src, array, index }
            | Self::AputShort { src, array, index } => {
                regs.push(u16::from(*src));
                regs.push(u16::from(*array));
                regs.push(u16::from(*index));
            }
            Self::Iget { dest, obj, .. }
            | Self::IgetWide { dest, obj, .. }
            | Self::IgetObject { dest, obj, .. }
            | Self::IgetBoolean { dest, obj, .. }
            | Self::IgetByte { dest, obj, .. }
            | Self::IgetChar { dest, obj, .. }
            | Self::IgetShort { dest, obj, .. } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*obj));
            }
            Self::Iput { src, obj, .. }
            | Self::IputWide { src, obj, .. }
            | Self::IputObject { src, obj, .. }
            | Self::IputBoolean { src, obj, .. }
            | Self::IputByte { src, obj, .. }
            | Self::IputChar { src, obj, .. }
            | Self::IputShort { src, obj, .. } => {
                regs.push(u16::from(*src));
                regs.push(u16::from(*obj));
            }
            Self::Sget { dest, .. }
            | Self::SgetWide { dest, .. }
            | Self::SgetObject { dest, .. }
            | Self::SgetBoolean { dest, .. }
            | Self::SgetByte { dest, .. }
            | Self::SgetChar { dest, .. }
            | Self::SgetShort { dest, .. } => regs.push(u16::from(*dest)),
            Self::Sput { src, .. }
            | Self::SputWide { src, .. }
            | Self::SputObject { src, .. }
            | Self::SputBoolean { src, .. }
            | Self::SputByte { src, .. }
            | Self::SputChar { src, .. }
            | Self::SputShort { src, .. } => regs.push(u16::from(*src)),
            Self::InvokeVirtual { args, .. }
            | Self::InvokeSuper { args, .. }
            | Self::InvokeDirect { args, .. }
            | Self::InvokeStatic { args, .. }
            | Self::InvokeInterface { args, .. } => {
                for register in args {
                    regs.push(u16::from(*register));
                }
            }
            Self::InvokeVirtualRange {
                first_reg, count, ..
            }
            | Self::InvokeSuperRange {
                first_reg, count, ..
            }
            | Self::InvokeDirectRange {
                first_reg, count, ..
            }
            | Self::InvokeStaticRange {
                first_reg, count, ..
            }
            | Self::InvokeInterfaceRange {
                first_reg, count, ..
            } => {
                for offset in 0..u16::from(*count) {
                    regs.push(*first_reg + offset);
                }
            }
            Self::InvokePolymorphic { args, .. } => {
                for register in args {
                    regs.push(u16::from(*register));
                }
            }
            Self::InvokePolymorphicRange {
                first_reg, count, ..
            } => {
                for offset in 0..u16::from(*count) {
                    regs.push(*first_reg + offset);
                }
            }
            Self::InvokeCustom { args, .. } => {
                for register in args {
                    regs.push(u16::from(*register));
                }
            }
            Self::InvokeCustomRange {
                first_reg, count, ..
            } => {
                for offset in 0..u16::from(*count) {
                    regs.push(*first_reg + offset);
                }
            }
            Self::NegInt { dest, src }
            | Self::NotInt { dest, src }
            | Self::NegLong { dest, src }
            | Self::NotLong { dest, src }
            | Self::NegFloat { dest, src }
            | Self::NegDouble { dest, src }
            | Self::IntToLong { dest, src }
            | Self::IntToFloat { dest, src }
            | Self::IntToDouble { dest, src }
            | Self::LongToInt { dest, src }
            | Self::LongToFloat { dest, src }
            | Self::LongToDouble { dest, src }
            | Self::FloatToInt { dest, src }
            | Self::FloatToLong { dest, src }
            | Self::FloatToDouble { dest, src }
            | Self::DoubleToInt { dest, src }
            | Self::DoubleToLong { dest, src }
            | Self::DoubleToFloat { dest, src }
            | Self::IntToByte { dest, src }
            | Self::IntToChar { dest, src }
            | Self::IntToShort { dest, src } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*src));
            }
            Self::AddInt { dest, a, b }
            | Self::SubInt { dest, a, b }
            | Self::MulInt { dest, a, b }
            | Self::DivInt { dest, a, b }
            | Self::RemInt { dest, a, b }
            | Self::AndInt { dest, a, b }
            | Self::OrInt { dest, a, b }
            | Self::XorInt { dest, a, b }
            | Self::ShlInt { dest, a, b }
            | Self::ShrInt { dest, a, b }
            | Self::UshrInt { dest, a, b }
            | Self::AddLong { dest, a, b }
            | Self::SubLong { dest, a, b }
            | Self::MulLong { dest, a, b }
            | Self::DivLong { dest, a, b }
            | Self::RemLong { dest, a, b }
            | Self::AndLong { dest, a, b }
            | Self::OrLong { dest, a, b }
            | Self::XorLong { dest, a, b }
            | Self::ShlLong { dest, a, b }
            | Self::ShrLong { dest, a, b }
            | Self::UshrLong { dest, a, b }
            | Self::AddFloat { dest, a, b }
            | Self::SubFloat { dest, a, b }
            | Self::MulFloat { dest, a, b }
            | Self::DivFloat { dest, a, b }
            | Self::RemFloat { dest, a, b }
            | Self::AddDouble { dest, a, b }
            | Self::SubDouble { dest, a, b }
            | Self::MulDouble { dest, a, b }
            | Self::DivDouble { dest, a, b }
            | Self::RemDouble { dest, a, b } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*a));
                regs.push(u16::from(*b));
            }
            Self::AddInt2Addr { dest_a, b }
            | Self::SubInt2Addr { dest_a, b }
            | Self::MulInt2Addr { dest_a, b }
            | Self::DivInt2Addr { dest_a, b }
            | Self::RemInt2Addr { dest_a, b }
            | Self::AndInt2Addr { dest_a, b }
            | Self::OrInt2Addr { dest_a, b }
            | Self::XorInt2Addr { dest_a, b }
            | Self::ShlInt2Addr { dest_a, b }
            | Self::ShrInt2Addr { dest_a, b }
            | Self::UshrInt2Addr { dest_a, b }
            | Self::AddLong2Addr { dest_a, b }
            | Self::SubLong2Addr { dest_a, b }
            | Self::MulLong2Addr { dest_a, b }
            | Self::DivLong2Addr { dest_a, b }
            | Self::RemLong2Addr { dest_a, b }
            | Self::AndLong2Addr { dest_a, b }
            | Self::OrLong2Addr { dest_a, b }
            | Self::XorLong2Addr { dest_a, b }
            | Self::ShlLong2Addr { dest_a, b }
            | Self::ShrLong2Addr { dest_a, b }
            | Self::UshrLong2Addr { dest_a, b }
            | Self::AddFloat2Addr { dest_a, b }
            | Self::SubFloat2Addr { dest_a, b }
            | Self::MulFloat2Addr { dest_a, b }
            | Self::DivFloat2Addr { dest_a, b }
            | Self::RemFloat2Addr { dest_a, b }
            | Self::AddDouble2Addr { dest_a, b }
            | Self::SubDouble2Addr { dest_a, b }
            | Self::MulDouble2Addr { dest_a, b }
            | Self::DivDouble2Addr { dest_a, b }
            | Self::RemDouble2Addr { dest_a, b } => {
                regs.push(u16::from(*dest_a));
                regs.push(u16::from(*b));
            }
            Self::AddIntLit16 { dest, src, .. }
            | Self::RsubIntLit16 { dest, src, .. }
            | Self::MulIntLit16 { dest, src, .. }
            | Self::DivIntLit16 { dest, src, .. }
            | Self::RemIntLit16 { dest, src, .. }
            | Self::AndIntLit16 { dest, src, .. }
            | Self::OrIntLit16 { dest, src, .. }
            | Self::XorIntLit16 { dest, src, .. } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*src));
            }
            Self::AddIntLit8 { dest, src, .. }
            | Self::RsubIntLit8 { dest, src, .. }
            | Self::MulIntLit8 { dest, src, .. }
            | Self::DivIntLit8 { dest, src, .. }
            | Self::RemIntLit8 { dest, src, .. }
            | Self::AndIntLit8 { dest, src, .. }
            | Self::OrIntLit8 { dest, src, .. }
            | Self::XorIntLit8 { dest, src, .. }
            | Self::ShlIntLit8 { dest, src, .. }
            | Self::ShrIntLit8 { dest, src, .. }
            | Self::UshrIntLit8 { dest, src, .. } => {
                regs.push(u16::from(*dest));
                regs.push(u16::from(*src));
            }
        }
        regs
    }
}
