// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::instruction::Instruction;

impl Instruction {
    /// Returns the number of outgoing argument registers used by invoke instructions,
    /// or 0 for non-invoke instructions. Used to compute `outs_size`.
    pub fn outgoing_arg_count(&self) -> u16 {
        match self {
            Self::InvokeVirtual { args, .. }
            | Self::InvokeSuper { args, .. }
            | Self::InvokeDirect { args, .. }
            | Self::InvokeStatic { args, .. }
            | Self::InvokeInterface { args, .. } => args.len() as u16,
            Self::InvokeVirtualRange { count, .. }
            | Self::InvokeSuperRange { count, .. }
            | Self::InvokeDirectRange { count, .. }
            | Self::InvokeStaticRange { count, .. }
            | Self::InvokeInterfaceRange { count, .. } => u16::from(*count),
            Self::InvokePolymorphic { args, .. } => args.len() as u16,
            Self::InvokePolymorphicRange { count, .. } => u16::from(*count),
            Self::InvokeCustom { args, .. } => args.len() as u16,
            Self::InvokeCustomRange { count, .. } => u16::from(*count),
            _ => 0,
        }
    }

    pub fn code_units(&self) -> u32 {
        match self {
            Self::Nop
            | Self::Move { .. }
            | Self::MoveWide { .. }
            | Self::MoveObject { .. }
            | Self::MoveResult { .. }
            | Self::MoveResultWide { .. }
            | Self::MoveResultObject { .. }
            | Self::MoveException { .. }
            | Self::ReturnVoid
            | Self::Return { .. }
            | Self::ReturnWide { .. }
            | Self::ReturnObject { .. }
            | Self::Const4 { .. }
            | Self::MonitorEnter { .. }
            | Self::MonitorExit { .. }
            | Self::ArrayLength { .. }
            | Self::Throw { .. }
            | Self::Goto { .. }
            | Self::NegInt { .. }
            | Self::NotInt { .. }
            | Self::NegLong { .. }
            | Self::NotLong { .. }
            | Self::NegFloat { .. }
            | Self::NegDouble { .. }
            | Self::IntToLong { .. }
            | Self::IntToFloat { .. }
            | Self::IntToDouble { .. }
            | Self::LongToInt { .. }
            | Self::LongToFloat { .. }
            | Self::LongToDouble { .. }
            | Self::FloatToInt { .. }
            | Self::FloatToLong { .. }
            | Self::FloatToDouble { .. }
            | Self::DoubleToInt { .. }
            | Self::DoubleToLong { .. }
            | Self::DoubleToFloat { .. }
            | Self::IntToByte { .. }
            | Self::IntToChar { .. }
            | Self::IntToShort { .. }
            | Self::AddInt2Addr { .. }
            | Self::SubInt2Addr { .. }
            | Self::MulInt2Addr { .. }
            | Self::DivInt2Addr { .. }
            | Self::RemInt2Addr { .. }
            | Self::AndInt2Addr { .. }
            | Self::OrInt2Addr { .. }
            | Self::XorInt2Addr { .. }
            | Self::ShlInt2Addr { .. }
            | Self::ShrInt2Addr { .. }
            | Self::UshrInt2Addr { .. }
            | Self::AddLong2Addr { .. }
            | Self::SubLong2Addr { .. }
            | Self::MulLong2Addr { .. }
            | Self::DivLong2Addr { .. }
            | Self::RemLong2Addr { .. }
            | Self::AndLong2Addr { .. }
            | Self::OrLong2Addr { .. }
            | Self::XorLong2Addr { .. }
            | Self::ShlLong2Addr { .. }
            | Self::ShrLong2Addr { .. }
            | Self::UshrLong2Addr { .. }
            | Self::AddFloat2Addr { .. }
            | Self::SubFloat2Addr { .. }
            | Self::MulFloat2Addr { .. }
            | Self::DivFloat2Addr { .. }
            | Self::RemFloat2Addr { .. }
            | Self::AddDouble2Addr { .. }
            | Self::SubDouble2Addr { .. }
            | Self::MulDouble2Addr { .. }
            | Self::DivDouble2Addr { .. }
            | Self::RemDouble2Addr { .. } => 1,
            Self::MoveFrom16 { .. }
            | Self::MoveWideFrom16 { .. }
            | Self::MoveObjectFrom16 { .. }
            | Self::Const16 { .. }
            | Self::ConstHigh16 { .. }
            | Self::ConstWide16 { .. }
            | Self::ConstWideHigh16 { .. }
            | Self::ConstString { .. }
            | Self::ConstClass { .. }
            | Self::ConstMethodHandle { .. }
            | Self::ConstMethodType { .. }
            | Self::CheckCast { .. }
            | Self::InstanceOf { .. }
            | Self::NewInstance { .. }
            | Self::NewArray { .. }
            | Self::Goto16 { .. }
            | Self::CmpLFloat { .. }
            | Self::CmpGFloat { .. }
            | Self::CmpLDouble { .. }
            | Self::CmpGDouble { .. }
            | Self::CmpLong { .. }
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
            | Self::Aget { .. }
            | Self::AgetWide { .. }
            | Self::AgetObject { .. }
            | Self::AgetBoolean { .. }
            | Self::AgetByte { .. }
            | Self::AgetChar { .. }
            | Self::AgetShort { .. }
            | Self::Aput { .. }
            | Self::AputWide { .. }
            | Self::AputObject { .. }
            | Self::AputBoolean { .. }
            | Self::AputByte { .. }
            | Self::AputChar { .. }
            | Self::AputShort { .. }
            | Self::Iget { .. }
            | Self::IgetWide { .. }
            | Self::IgetObject { .. }
            | Self::IgetBoolean { .. }
            | Self::IgetByte { .. }
            | Self::IgetChar { .. }
            | Self::IgetShort { .. }
            | Self::Iput { .. }
            | Self::IputWide { .. }
            | Self::IputObject { .. }
            | Self::IputBoolean { .. }
            | Self::IputByte { .. }
            | Self::IputChar { .. }
            | Self::IputShort { .. }
            | Self::Sget { .. }
            | Self::SgetWide { .. }
            | Self::SgetObject { .. }
            | Self::SgetBoolean { .. }
            | Self::SgetByte { .. }
            | Self::SgetChar { .. }
            | Self::SgetShort { .. }
            | Self::Sput { .. }
            | Self::SputWide { .. }
            | Self::SputObject { .. }
            | Self::SputBoolean { .. }
            | Self::SputByte { .. }
            | Self::SputChar { .. }
            | Self::SputShort { .. }
            | Self::AddInt { .. }
            | Self::SubInt { .. }
            | Self::MulInt { .. }
            | Self::DivInt { .. }
            | Self::RemInt { .. }
            | Self::AndInt { .. }
            | Self::OrInt { .. }
            | Self::XorInt { .. }
            | Self::ShlInt { .. }
            | Self::ShrInt { .. }
            | Self::UshrInt { .. }
            | Self::AddLong { .. }
            | Self::SubLong { .. }
            | Self::MulLong { .. }
            | Self::DivLong { .. }
            | Self::RemLong { .. }
            | Self::AndLong { .. }
            | Self::OrLong { .. }
            | Self::XorLong { .. }
            | Self::ShlLong { .. }
            | Self::ShrLong { .. }
            | Self::UshrLong { .. }
            | Self::AddFloat { .. }
            | Self::SubFloat { .. }
            | Self::MulFloat { .. }
            | Self::DivFloat { .. }
            | Self::RemFloat { .. }
            | Self::AddDouble { .. }
            | Self::SubDouble { .. }
            | Self::MulDouble { .. }
            | Self::DivDouble { .. }
            | Self::RemDouble { .. }
            | Self::AddIntLit16 { .. }
            | Self::RsubIntLit16 { .. }
            | Self::MulIntLit16 { .. }
            | Self::DivIntLit16 { .. }
            | Self::RemIntLit16 { .. }
            | Self::AndIntLit16 { .. }
            | Self::OrIntLit16 { .. }
            | Self::XorIntLit16 { .. }
            | Self::AddIntLit8 { .. }
            | Self::RsubIntLit8 { .. }
            | Self::MulIntLit8 { .. }
            | Self::DivIntLit8 { .. }
            | Self::RemIntLit8 { .. }
            | Self::AndIntLit8 { .. }
            | Self::OrIntLit8 { .. }
            | Self::XorIntLit8 { .. }
            | Self::ShlIntLit8 { .. }
            | Self::ShrIntLit8 { .. }
            | Self::UshrIntLit8 { .. } => 2,
            Self::Move16 { .. }
            | Self::MoveWide16 { .. }
            | Self::MoveObject16 { .. }
            | Self::ConstWide32 { .. }
            | Self::Const { .. }
            | Self::ConstStringJumbo { .. }
            | Self::FillArrayData { .. }
            | Self::Goto32 { .. }
            | Self::PackedSwitch { .. }
            | Self::SparseSwitch { .. }
            | Self::FilledNewArray { .. }
            | Self::FilledNewArrayRange { .. }
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
            | Self::InvokeCustom { .. }
            | Self::InvokeCustomRange { .. } => 3,
            Self::InvokePolymorphic { .. } | Self::InvokePolymorphicRange { .. } => 4,
            Self::ConstWide { .. } => 5,
            Self::PackedSwitchPayload { targets, .. } => (1 + 1 + 2 + targets.len() * 2) as u32,
            Self::SparseSwitchPayload {
                keys_and_targets, ..
            } => (1 + 1 + keys_and_targets.len() * 4) as u32,
            Self::FillArrayDataPayload { data, .. } => (4 + data.len().div_ceil(2)) as u32,
            Self::RawInstruction { code_units } => code_units.len() as u32,
        }
    }

    pub fn opcode(&self) -> Option<u16> {
        Some(match self {
            Self::Nop => 0x00,
            Self::Move { .. } => 0x01,
            Self::MoveFrom16 { .. } => 0x02,
            Self::Move16 { .. } => 0x03,
            Self::MoveWide { .. } => 0x04,
            Self::MoveWideFrom16 { .. } => 0x05,
            Self::MoveWide16 { .. } => 0x06,
            Self::MoveObject { .. } => 0x07,
            Self::MoveObjectFrom16 { .. } => 0x08,
            Self::MoveObject16 { .. } => 0x09,
            Self::MoveResult { .. } => 0x0a,
            Self::MoveResultWide { .. } => 0x0b,
            Self::MoveResultObject { .. } => 0x0c,
            Self::MoveException { .. } => 0x0d,
            Self::ReturnVoid => 0x0e,
            Self::Return { .. } => 0x0f,
            Self::ReturnWide { .. } => 0x10,
            Self::ReturnObject { .. } => 0x11,
            Self::Const4 { .. } => 0x12,
            Self::Const16 { .. } => 0x13,
            Self::Const { .. } => 0x14,
            Self::ConstHigh16 { .. } => 0x15,
            Self::ConstWide16 { .. } => 0x16,
            Self::ConstWide32 { .. } => 0x17,
            Self::ConstWide { .. } => 0x18,
            Self::ConstWideHigh16 { .. } => 0x19,
            Self::ConstString { .. } => 0x1a,
            Self::ConstStringJumbo { .. } => 0x1b,
            Self::ConstClass { .. } => 0x1c,
            Self::MonitorEnter { .. } => 0x1d,
            Self::MonitorExit { .. } => 0x1e,
            Self::CheckCast { .. } => 0x1f,
            Self::InstanceOf { .. } => 0x20,
            Self::ArrayLength { .. } => 0x21,
            Self::NewInstance { .. } => 0x22,
            Self::NewArray { .. } => 0x23,
            Self::FilledNewArray { .. } => 0x24,
            Self::FilledNewArrayRange { .. } => 0x25,
            Self::FillArrayData { .. } => 0x26,
            Self::Throw { .. } => 0x27,
            Self::Goto { .. } => 0x28,
            Self::Goto16 { .. } => 0x29,
            Self::Goto32 { .. } => 0x2a,
            Self::PackedSwitch { .. } => 0x2b,
            Self::SparseSwitch { .. } => 0x2c,
            Self::CmpLFloat { .. } => 0x2d,
            Self::CmpGFloat { .. } => 0x2e,
            Self::CmpLDouble { .. } => 0x2f,
            Self::CmpGDouble { .. } => 0x30,
            Self::CmpLong { .. } => 0x31,
            Self::IfEq { .. } => 0x32,
            Self::IfNe { .. } => 0x33,
            Self::IfLt { .. } => 0x34,
            Self::IfGe { .. } => 0x35,
            Self::IfGt { .. } => 0x36,
            Self::IfLe { .. } => 0x37,
            Self::IfEqz { .. } => 0x38,
            Self::IfNez { .. } => 0x39,
            Self::IfLtz { .. } => 0x3a,
            Self::IfGez { .. } => 0x3b,
            Self::IfGtz { .. } => 0x3c,
            Self::IfLez { .. } => 0x3d,
            Self::Aget { .. } => 0x44,
            Self::AgetWide { .. } => 0x45,
            Self::AgetObject { .. } => 0x46,
            Self::AgetBoolean { .. } => 0x47,
            Self::AgetByte { .. } => 0x48,
            Self::AgetChar { .. } => 0x49,
            Self::AgetShort { .. } => 0x4a,
            Self::Aput { .. } => 0x4b,
            Self::AputWide { .. } => 0x4c,
            Self::AputObject { .. } => 0x4d,
            Self::AputBoolean { .. } => 0x4e,
            Self::AputByte { .. } => 0x4f,
            Self::AputChar { .. } => 0x50,
            Self::AputShort { .. } => 0x51,
            Self::Iget { .. } => 0x52,
            Self::IgetWide { .. } => 0x53,
            Self::IgetObject { .. } => 0x54,
            Self::IgetBoolean { .. } => 0x55,
            Self::IgetByte { .. } => 0x56,
            Self::IgetChar { .. } => 0x57,
            Self::IgetShort { .. } => 0x58,
            Self::Iput { .. } => 0x59,
            Self::IputWide { .. } => 0x5a,
            Self::IputObject { .. } => 0x5b,
            Self::IputBoolean { .. } => 0x5c,
            Self::IputByte { .. } => 0x5d,
            Self::IputChar { .. } => 0x5e,
            Self::IputShort { .. } => 0x5f,
            Self::Sget { .. } => 0x60,
            Self::SgetWide { .. } => 0x61,
            Self::SgetObject { .. } => 0x62,
            Self::SgetBoolean { .. } => 0x63,
            Self::SgetByte { .. } => 0x64,
            Self::SgetChar { .. } => 0x65,
            Self::SgetShort { .. } => 0x66,
            Self::Sput { .. } => 0x67,
            Self::SputWide { .. } => 0x68,
            Self::SputObject { .. } => 0x69,
            Self::SputBoolean { .. } => 0x6a,
            Self::SputByte { .. } => 0x6b,
            Self::SputChar { .. } => 0x6c,
            Self::SputShort { .. } => 0x6d,
            Self::InvokeVirtual { .. } => 0x6e,
            Self::InvokeSuper { .. } => 0x6f,
            Self::InvokeDirect { .. } => 0x70,
            Self::InvokeStatic { .. } => 0x71,
            Self::InvokeInterface { .. } => 0x72,
            Self::InvokeVirtualRange { .. } => 0x74,
            Self::InvokeSuperRange { .. } => 0x75,
            Self::InvokeDirectRange { .. } => 0x76,
            Self::InvokeStaticRange { .. } => 0x77,
            Self::InvokeInterfaceRange { .. } => 0x78,
            Self::NegInt { .. } => 0x7b,
            Self::NotInt { .. } => 0x7c,
            Self::NegLong { .. } => 0x7d,
            Self::NotLong { .. } => 0x7e,
            Self::NegFloat { .. } => 0x7f,
            Self::NegDouble { .. } => 0x80,
            Self::IntToLong { .. } => 0x81,
            Self::IntToFloat { .. } => 0x82,
            Self::IntToDouble { .. } => 0x83,
            Self::LongToInt { .. } => 0x84,
            Self::LongToFloat { .. } => 0x85,
            Self::LongToDouble { .. } => 0x86,
            Self::FloatToInt { .. } => 0x87,
            Self::FloatToLong { .. } => 0x88,
            Self::FloatToDouble { .. } => 0x89,
            Self::DoubleToInt { .. } => 0x8a,
            Self::DoubleToLong { .. } => 0x8b,
            Self::DoubleToFloat { .. } => 0x8c,
            Self::IntToByte { .. } => 0x8d,
            Self::IntToChar { .. } => 0x8e,
            Self::IntToShort { .. } => 0x8f,
            Self::AddInt { .. } => 0x90,
            Self::SubInt { .. } => 0x91,
            Self::MulInt { .. } => 0x92,
            Self::DivInt { .. } => 0x93,
            Self::RemInt { .. } => 0x94,
            Self::AndInt { .. } => 0x95,
            Self::OrInt { .. } => 0x96,
            Self::XorInt { .. } => 0x97,
            Self::ShlInt { .. } => 0x98,
            Self::ShrInt { .. } => 0x99,
            Self::UshrInt { .. } => 0x9a,
            Self::AddLong { .. } => 0x9b,
            Self::SubLong { .. } => 0x9c,
            Self::MulLong { .. } => 0x9d,
            Self::DivLong { .. } => 0x9e,
            Self::RemLong { .. } => 0x9f,
            Self::AndLong { .. } => 0xa0,
            Self::OrLong { .. } => 0xa1,
            Self::XorLong { .. } => 0xa2,
            Self::ShlLong { .. } => 0xa3,
            Self::ShrLong { .. } => 0xa4,
            Self::UshrLong { .. } => 0xa5,
            Self::AddFloat { .. } => 0xa6,
            Self::SubFloat { .. } => 0xa7,
            Self::MulFloat { .. } => 0xa8,
            Self::DivFloat { .. } => 0xa9,
            Self::RemFloat { .. } => 0xaa,
            Self::AddDouble { .. } => 0xab,
            Self::SubDouble { .. } => 0xac,
            Self::MulDouble { .. } => 0xad,
            Self::DivDouble { .. } => 0xae,
            Self::RemDouble { .. } => 0xaf,
            Self::AddInt2Addr { .. } => 0xb0,
            Self::SubInt2Addr { .. } => 0xb1,
            Self::MulInt2Addr { .. } => 0xb2,
            Self::DivInt2Addr { .. } => 0xb3,
            Self::RemInt2Addr { .. } => 0xb4,
            Self::AndInt2Addr { .. } => 0xb5,
            Self::OrInt2Addr { .. } => 0xb6,
            Self::XorInt2Addr { .. } => 0xb7,
            Self::ShlInt2Addr { .. } => 0xb8,
            Self::ShrInt2Addr { .. } => 0xb9,
            Self::UshrInt2Addr { .. } => 0xba,
            Self::AddLong2Addr { .. } => 0xbb,
            Self::SubLong2Addr { .. } => 0xbc,
            Self::MulLong2Addr { .. } => 0xbd,
            Self::DivLong2Addr { .. } => 0xbe,
            Self::RemLong2Addr { .. } => 0xbf,
            Self::AndLong2Addr { .. } => 0xc0,
            Self::OrLong2Addr { .. } => 0xc1,
            Self::XorLong2Addr { .. } => 0xc2,
            Self::ShlLong2Addr { .. } => 0xc3,
            Self::ShrLong2Addr { .. } => 0xc4,
            Self::UshrLong2Addr { .. } => 0xc5,
            Self::AddFloat2Addr { .. } => 0xc6,
            Self::SubFloat2Addr { .. } => 0xc7,
            Self::MulFloat2Addr { .. } => 0xc8,
            Self::DivFloat2Addr { .. } => 0xc9,
            Self::RemFloat2Addr { .. } => 0xca,
            Self::AddDouble2Addr { .. } => 0xcb,
            Self::SubDouble2Addr { .. } => 0xcc,
            Self::MulDouble2Addr { .. } => 0xcd,
            Self::DivDouble2Addr { .. } => 0xce,
            Self::RemDouble2Addr { .. } => 0xcf,
            Self::AddIntLit16 { .. } => 0xd0,
            Self::RsubIntLit16 { .. } => 0xd1,
            Self::MulIntLit16 { .. } => 0xd2,
            Self::DivIntLit16 { .. } => 0xd3,
            Self::RemIntLit16 { .. } => 0xd4,
            Self::AndIntLit16 { .. } => 0xd5,
            Self::OrIntLit16 { .. } => 0xd6,
            Self::XorIntLit16 { .. } => 0xd7,
            Self::AddIntLit8 { .. } => 0xd8,
            Self::RsubIntLit8 { .. } => 0xd9,
            Self::MulIntLit8 { .. } => 0xda,
            Self::DivIntLit8 { .. } => 0xdb,
            Self::RemIntLit8 { .. } => 0xdc,
            Self::AndIntLit8 { .. } => 0xdd,
            Self::OrIntLit8 { .. } => 0xde,
            Self::XorIntLit8 { .. } => 0xdf,
            Self::ShlIntLit8 { .. } => 0xe0,
            Self::ShrIntLit8 { .. } => 0xe1,
            Self::UshrIntLit8 { .. } => 0xe2,
            Self::InvokePolymorphic { .. } => 0xfa,
            Self::InvokePolymorphicRange { .. } => 0xfb,
            Self::InvokeCustom { .. } => 0xfc,
            Self::InvokeCustomRange { .. } => 0xfd,
            Self::ConstMethodHandle { .. } => 0xfe,
            Self::ConstMethodType { .. } => 0xff,
            Self::PackedSwitchPayload { .. } => 0x0100,
            Self::SparseSwitchPayload { .. } => 0x0200,
            Self::FillArrayDataPayload { .. } => 0x0300,
            Self::RawInstruction { .. } => return None,
        })
    }
}
