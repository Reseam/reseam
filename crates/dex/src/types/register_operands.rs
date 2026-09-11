// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed register operands used by register allocation and instruction lowering.

use super::instruction::Instruction;
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegisterKind {
    Value,
    Object,
    Wide,
    Unknown,
}

impl RegisterKind {
    pub fn words(self) -> u16 {
        if self == Self::Wide {
            2
        } else {
            1
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    Read,
    Write,
    ReadWrite,
}

pub(crate) struct Operand {
    pub register: u16,
    pub kind: RegisterKind,
    pub access: Access,
    pub max: u16,
}

pub(crate) fn map_operands(
    insn: &mut Instruction,
    mut map: impl FnMut(Operand) -> Result<u16>,
) -> Result<()> {
    use Access::*;
    use RegisterKind::*;
    match insn {
        Instruction::Move { dest, src, .. }
        | Instruction::NegInt { dest, src, .. }
        | Instruction::NotInt { dest, src, .. }
        | Instruction::NegFloat { dest, src, .. }
        | Instruction::IntToFloat { dest, src, .. }
        | Instruction::FloatToInt { dest, src, .. }
        | Instruction::IntToByte { dest, src, .. }
        | Instruction::IntToChar { dest, src, .. }
        | Instruction::IntToShort { dest, src, .. }
        | Instruction::AddIntLit16 { dest, src, .. }
        | Instruction::RsubIntLit16 { dest, src, .. }
        | Instruction::MulIntLit16 { dest, src, .. }
        | Instruction::DivIntLit16 { dest, src, .. }
        | Instruction::RemIntLit16 { dest, src, .. }
        | Instruction::AndIntLit16 { dest, src, .. }
        | Instruction::OrIntLit16 { dest, src, .. }
        | Instruction::XorIntLit16 { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: 15,
            })? as u8;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Value,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::MoveFrom16 { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Value,
                access: Read,
                max: u16::MAX,
            })? as u16;
        }
        Instruction::Move16 { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: u16::MAX,
            })? as u16;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Value,
                access: Read,
                max: u16::MAX,
            })? as u16;
        }
        Instruction::MoveWide { dest, src, .. }
        | Instruction::NegLong { dest, src, .. }
        | Instruction::NotLong { dest, src, .. }
        | Instruction::NegDouble { dest, src, .. }
        | Instruction::LongToDouble { dest, src, .. }
        | Instruction::DoubleToLong { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Wide,
                access: Write,
                max: 15,
            })? as u8;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Wide,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::MoveWideFrom16 { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Wide,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Wide,
                access: Read,
                max: u16::MAX,
            })? as u16;
        }
        Instruction::MoveWide16 { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Wide,
                access: Write,
                max: u16::MAX,
            })? as u16;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Wide,
                access: Read,
                max: u16::MAX,
            })? as u16;
        }
        Instruction::MoveObject { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Object,
                access: Write,
                max: 15,
            })? as u8;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::MoveObjectFrom16 { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Object,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Object,
                access: Read,
                max: u16::MAX,
            })? as u16;
        }
        Instruction::MoveObject16 { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Object,
                access: Write,
                max: u16::MAX,
            })? as u16;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Object,
                access: Read,
                max: u16::MAX,
            })? as u16;
        }
        Instruction::MoveResult { dest, .. }
        | Instruction::Const16 { dest, .. }
        | Instruction::Const { dest, .. }
        | Instruction::ConstHigh16 { dest, .. }
        | Instruction::Sget { dest, .. }
        | Instruction::SgetBoolean { dest, .. }
        | Instruction::SgetByte { dest, .. }
        | Instruction::SgetChar { dest, .. }
        | Instruction::SgetShort { dest, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::MoveResultWide { dest, .. }
        | Instruction::ConstWide16 { dest, .. }
        | Instruction::ConstWide32 { dest, .. }
        | Instruction::ConstWide { dest, .. }
        | Instruction::ConstWideHigh16 { dest, .. }
        | Instruction::SgetWide { dest, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Wide,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::MoveResultObject { dest, .. }
        | Instruction::MoveException { dest, .. }
        | Instruction::ConstString { dest, .. }
        | Instruction::ConstStringJumbo { dest, .. }
        | Instruction::ConstClass { dest, .. }
        | Instruction::NewInstance { dest, .. }
        | Instruction::SgetObject { dest, .. }
        | Instruction::ConstMethodHandle { dest, .. }
        | Instruction::ConstMethodType { dest, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Object,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::Return { src, .. }
        | Instruction::Sput { src, .. }
        | Instruction::SputBoolean { src, .. }
        | Instruction::SputByte { src, .. }
        | Instruction::SputChar { src, .. }
        | Instruction::SputShort { src, .. } => {
            *src = map(Operand {
                register: u16::from(*src),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::ReturnWide { src, .. } | Instruction::SputWide { src, .. } => {
            *src = map(Operand {
                register: u16::from(*src),
                kind: Wide,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::ReturnObject { src, .. } | Instruction::SputObject { src, .. } => {
            *src = map(Operand {
                register: u16::from(*src),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::Const4 { dest, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: 15,
            })? as u8;
        }
        Instruction::MonitorEnter { ref_, .. } | Instruction::MonitorExit { ref_, .. } => {
            *ref_ = map(Operand {
                register: u16::from(*ref_),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::CheckCast { ref_, .. } => {
            *ref_ = map(Operand {
                register: u16::from(*ref_),
                kind: Object,
                access: ReadWrite,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::InstanceOf { dest, ref_, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: 15,
            })? as u8;
            *ref_ = map(Operand {
                register: u16::from(*ref_),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::ArrayLength { dest, array, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: 15,
            })? as u8;
            *array = map(Operand {
                register: u16::from(*array),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::NewArray { dest, size, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Object,
                access: Write,
                max: 15,
            })? as u8;
            *size = map(Operand {
                register: u16::from(*size),
                kind: Value,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::FillArrayData { array, .. } => {
            *array = map(Operand {
                register: u16::from(*array),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::Throw { exception, .. } => {
            *exception = map(Operand {
                register: u16::from(*exception),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::PackedSwitch { test, .. } | Instruction::SparseSwitch { test, .. } => {
            *test = map(Operand {
                register: u16::from(*test),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::CmpLFloat { dest, a, b, .. }
        | Instruction::CmpGFloat { dest, a, b, .. }
        | Instruction::AddInt { dest, a, b, .. }
        | Instruction::SubInt { dest, a, b, .. }
        | Instruction::MulInt { dest, a, b, .. }
        | Instruction::DivInt { dest, a, b, .. }
        | Instruction::RemInt { dest, a, b, .. }
        | Instruction::AndInt { dest, a, b, .. }
        | Instruction::OrInt { dest, a, b, .. }
        | Instruction::XorInt { dest, a, b, .. }
        | Instruction::ShlInt { dest, a, b, .. }
        | Instruction::ShrInt { dest, a, b, .. }
        | Instruction::UshrInt { dest, a, b, .. }
        | Instruction::AddFloat { dest, a, b, .. }
        | Instruction::SubFloat { dest, a, b, .. }
        | Instruction::MulFloat { dest, a, b, .. }
        | Instruction::DivFloat { dest, a, b, .. }
        | Instruction::RemFloat { dest, a, b, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *a = map(Operand {
                register: u16::from(*a),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *b = map(Operand {
                register: u16::from(*b),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::CmpLDouble { dest, a, b, .. }
        | Instruction::CmpGDouble { dest, a, b, .. }
        | Instruction::CmpLong { dest, a, b, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *a = map(Operand {
                register: u16::from(*a),
                kind: Wide,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *b = map(Operand {
                register: u16::from(*b),
                kind: Wide,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::IfEq { a, b, .. } | Instruction::IfNe { a, b, .. } => {
            *a = map(Operand {
                register: u16::from(*a),
                kind: Unknown,
                access: Read,
                max: 15,
            })? as u8;
            *b = map(Operand {
                register: u16::from(*b),
                kind: Unknown,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::IfLt { a, b, .. }
        | Instruction::IfGe { a, b, .. }
        | Instruction::IfGt { a, b, .. }
        | Instruction::IfLe { a, b, .. } => {
            *a = map(Operand {
                register: u16::from(*a),
                kind: Value,
                access: Read,
                max: 15,
            })? as u8;
            *b = map(Operand {
                register: u16::from(*b),
                kind: Value,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::IfEqz { a, .. } | Instruction::IfNez { a, .. } => {
            *a = map(Operand {
                register: u16::from(*a),
                kind: Unknown,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::IfLtz { a, .. }
        | Instruction::IfGez { a, .. }
        | Instruction::IfGtz { a, .. }
        | Instruction::IfLez { a, .. } => {
            *a = map(Operand {
                register: u16::from(*a),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::Aget {
            dest, array, index, ..
        }
        | Instruction::AgetBoolean {
            dest, array, index, ..
        }
        | Instruction::AgetByte {
            dest, array, index, ..
        }
        | Instruction::AgetChar {
            dest, array, index, ..
        }
        | Instruction::AgetShort {
            dest, array, index, ..
        } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *array = map(Operand {
                register: u16::from(*array),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *index = map(Operand {
                register: u16::from(*index),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::AgetWide {
            dest, array, index, ..
        } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Wide,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *array = map(Operand {
                register: u16::from(*array),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *index = map(Operand {
                register: u16::from(*index),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::AgetObject {
            dest, array, index, ..
        } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Object,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *array = map(Operand {
                register: u16::from(*array),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *index = map(Operand {
                register: u16::from(*index),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::Aput {
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
            *src = map(Operand {
                register: u16::from(*src),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *array = map(Operand {
                register: u16::from(*array),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *index = map(Operand {
                register: u16::from(*index),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::AputWide {
            src, array, index, ..
        } => {
            *src = map(Operand {
                register: u16::from(*src),
                kind: Wide,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *array = map(Operand {
                register: u16::from(*array),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *index = map(Operand {
                register: u16::from(*index),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::AputObject {
            src, array, index, ..
        } => {
            *src = map(Operand {
                register: u16::from(*src),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *array = map(Operand {
                register: u16::from(*array),
                kind: Object,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *index = map(Operand {
                register: u16::from(*index),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::Iget { dest, obj, .. }
        | Instruction::IgetBoolean { dest, obj, .. }
        | Instruction::IgetByte { dest, obj, .. }
        | Instruction::IgetChar { dest, obj, .. }
        | Instruction::IgetShort { dest, obj, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: 15,
            })? as u8;
            *obj = map(Operand {
                register: u16::from(*obj),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::IgetWide { dest, obj, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Wide,
                access: Write,
                max: 15,
            })? as u8;
            *obj = map(Operand {
                register: u16::from(*obj),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::IgetObject { dest, obj, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Object,
                access: Write,
                max: 15,
            })? as u8;
            *obj = map(Operand {
                register: u16::from(*obj),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::Iput { src, obj, .. }
        | Instruction::IputBoolean { src, obj, .. }
        | Instruction::IputByte { src, obj, .. }
        | Instruction::IputChar { src, obj, .. }
        | Instruction::IputShort { src, obj, .. } => {
            *src = map(Operand {
                register: u16::from(*src),
                kind: Value,
                access: Read,
                max: 15,
            })? as u8;
            *obj = map(Operand {
                register: u16::from(*obj),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::IputWide { src, obj, .. } => {
            *src = map(Operand {
                register: u16::from(*src),
                kind: Wide,
                access: Read,
                max: 15,
            })? as u8;
            *obj = map(Operand {
                register: u16::from(*obj),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::IputObject { src, obj, .. } => {
            *src = map(Operand {
                register: u16::from(*src),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
            *obj = map(Operand {
                register: u16::from(*obj),
                kind: Object,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::IntToLong { dest, src, .. }
        | Instruction::IntToDouble { dest, src, .. }
        | Instruction::FloatToLong { dest, src, .. }
        | Instruction::FloatToDouble { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Wide,
                access: Write,
                max: 15,
            })? as u8;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Value,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::LongToInt { dest, src, .. }
        | Instruction::LongToFloat { dest, src, .. }
        | Instruction::DoubleToInt { dest, src, .. }
        | Instruction::DoubleToFloat { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: 15,
            })? as u8;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Wide,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::AddLong { dest, a, b, .. }
        | Instruction::SubLong { dest, a, b, .. }
        | Instruction::MulLong { dest, a, b, .. }
        | Instruction::DivLong { dest, a, b, .. }
        | Instruction::RemLong { dest, a, b, .. }
        | Instruction::AndLong { dest, a, b, .. }
        | Instruction::OrLong { dest, a, b, .. }
        | Instruction::XorLong { dest, a, b, .. }
        | Instruction::AddDouble { dest, a, b, .. }
        | Instruction::SubDouble { dest, a, b, .. }
        | Instruction::MulDouble { dest, a, b, .. }
        | Instruction::DivDouble { dest, a, b, .. }
        | Instruction::RemDouble { dest, a, b, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Wide,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *a = map(Operand {
                register: u16::from(*a),
                kind: Wide,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *b = map(Operand {
                register: u16::from(*b),
                kind: Wide,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::ShlLong { dest, a, b, .. }
        | Instruction::ShrLong { dest, a, b, .. }
        | Instruction::UshrLong { dest, a, b, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Wide,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *a = map(Operand {
                register: u16::from(*a),
                kind: Wide,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
            *b = map(Operand {
                register: u16::from(*b),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::AddInt2Addr { dest_a, b, .. }
        | Instruction::SubInt2Addr { dest_a, b, .. }
        | Instruction::MulInt2Addr { dest_a, b, .. }
        | Instruction::DivInt2Addr { dest_a, b, .. }
        | Instruction::RemInt2Addr { dest_a, b, .. }
        | Instruction::AndInt2Addr { dest_a, b, .. }
        | Instruction::OrInt2Addr { dest_a, b, .. }
        | Instruction::XorInt2Addr { dest_a, b, .. }
        | Instruction::ShlInt2Addr { dest_a, b, .. }
        | Instruction::ShrInt2Addr { dest_a, b, .. }
        | Instruction::UshrInt2Addr { dest_a, b, .. }
        | Instruction::AddFloat2Addr { dest_a, b, .. }
        | Instruction::SubFloat2Addr { dest_a, b, .. }
        | Instruction::MulFloat2Addr { dest_a, b, .. }
        | Instruction::DivFloat2Addr { dest_a, b, .. }
        | Instruction::RemFloat2Addr { dest_a, b, .. } => {
            *dest_a = map(Operand {
                register: u16::from(*dest_a),
                kind: Value,
                access: ReadWrite,
                max: 15,
            })? as u8;
            *b = map(Operand {
                register: u16::from(*b),
                kind: Value,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::AddLong2Addr { dest_a, b, .. }
        | Instruction::SubLong2Addr { dest_a, b, .. }
        | Instruction::MulLong2Addr { dest_a, b, .. }
        | Instruction::DivLong2Addr { dest_a, b, .. }
        | Instruction::RemLong2Addr { dest_a, b, .. }
        | Instruction::AndLong2Addr { dest_a, b, .. }
        | Instruction::OrLong2Addr { dest_a, b, .. }
        | Instruction::XorLong2Addr { dest_a, b, .. }
        | Instruction::AddDouble2Addr { dest_a, b, .. }
        | Instruction::SubDouble2Addr { dest_a, b, .. }
        | Instruction::MulDouble2Addr { dest_a, b, .. }
        | Instruction::DivDouble2Addr { dest_a, b, .. }
        | Instruction::RemDouble2Addr { dest_a, b, .. } => {
            *dest_a = map(Operand {
                register: u16::from(*dest_a),
                kind: Wide,
                access: ReadWrite,
                max: 15,
            })? as u8;
            *b = map(Operand {
                register: u16::from(*b),
                kind: Wide,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::ShlLong2Addr { dest_a, b, .. }
        | Instruction::ShrLong2Addr { dest_a, b, .. }
        | Instruction::UshrLong2Addr { dest_a, b, .. } => {
            *dest_a = map(Operand {
                register: u16::from(*dest_a),
                kind: Wide,
                access: ReadWrite,
                max: 15,
            })? as u8;
            *b = map(Operand {
                register: u16::from(*b),
                kind: Value,
                access: Read,
                max: 15,
            })? as u8;
        }
        Instruction::AddIntLit8 { dest, src, .. }
        | Instruction::RsubIntLit8 { dest, src, .. }
        | Instruction::MulIntLit8 { dest, src, .. }
        | Instruction::DivIntLit8 { dest, src, .. }
        | Instruction::RemIntLit8 { dest, src, .. }
        | Instruction::AndIntLit8 { dest, src, .. }
        | Instruction::OrIntLit8 { dest, src, .. }
        | Instruction::XorIntLit8 { dest, src, .. }
        | Instruction::ShlIntLit8 { dest, src, .. }
        | Instruction::ShrIntLit8 { dest, src, .. }
        | Instruction::UshrIntLit8 { dest, src, .. } => {
            *dest = map(Operand {
                register: u16::from(*dest),
                kind: Value,
                access: Write,
                max: u8::MAX as u16,
            })? as u8;
            *src = map(Operand {
                register: u16::from(*src),
                kind: Value,
                access: Read,
                max: u8::MAX as u16,
            })? as u8;
        }
        Instruction::FilledNewArray { .. }
        | Instruction::FilledNewArrayRange { .. }
        | Instruction::Goto { .. }
        | Instruction::Goto16 { .. }
        | Instruction::Goto32 { .. }
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
        | Instruction::RawInstruction { .. }
        | Instruction::Nop
        | Instruction::ReturnVoid
        | Instruction::PackedSwitchPayload(_)
        | Instruction::SparseSwitchPayload(_)
        | Instruction::FillArrayDataPayload(_) => {}
    }
    Ok(())
}
