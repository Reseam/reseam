use crate::model::instruction::Instruction;

/// Pattern element used by opcode sequence searches.
#[derive(Debug, Clone)]
pub enum InstructionPattern {
    /// Match any instruction.
    Any,
    /// Match by opcode name (discriminant match).
    Opcode(OpcodeMatcher),
}

/// Opcode matcher that ignores operand values.
#[derive(Debug, Clone, Copy)]
pub enum OpcodeMatcher {
    Nop,
    Move,
    MoveFrom16,
    Move16,
    MoveWide,
    MoveWideFrom16,
    MoveWide16,
    MoveObject,
    MoveObjectFrom16,
    MoveObject16,
    MoveResult,
    MoveResultWide,
    MoveResultObject,
    MoveException,
    ReturnVoid,
    Return,
    ReturnWide,
    ReturnObject,
    Const4,
    Const16,
    Const,
    ConstHigh16,
    ConstWide16,
    ConstWide32,
    ConstWide,
    ConstWideHigh16,
    ConstString,
    ConstStringJumbo,
    ConstClass,
    MonitorEnter,
    MonitorExit,
    CheckCast,
    InstanceOf,
    ArrayLength,
    NewInstance,
    NewArray,
    FilledNewArray,
    FilledNewArrayRange,
    FillArrayData,
    Throw,
    Goto,
    Goto16,
    Goto32,
    PackedSwitch,
    SparseSwitch,
    CmpLFloat,
    CmpGFloat,
    CmpLDouble,
    CmpGDouble,
    CmpLong,
    IfEq,
    IfNe,
    IfLt,
    IfGe,
    IfGt,
    IfLe,
    IfEqz,
    IfNez,
    IfLtz,
    IfGez,
    IfGtz,
    IfLez,
    Aget,
    AgetWide,
    AgetObject,
    AgetBoolean,
    AgetByte,
    AgetChar,
    AgetShort,
    Aput,
    AputWide,
    AputObject,
    AputBoolean,
    AputByte,
    AputChar,
    AputShort,
    Iget,
    IgetWide,
    IgetObject,
    IgetBoolean,
    IgetByte,
    IgetChar,
    IgetShort,
    Iput,
    IputWide,
    IputObject,
    IputBoolean,
    IputByte,
    IputChar,
    IputShort,
    Sget,
    SgetWide,
    SgetObject,
    SgetBoolean,
    SgetByte,
    SgetChar,
    SgetShort,
    Sput,
    SputWide,
    SputObject,
    SputBoolean,
    SputByte,
    SputChar,
    SputShort,
    InvokeVirtual,
    InvokeSuper,
    InvokeDirect,
    InvokeStatic,
    InvokeInterface,
    InvokeVirtualRange,
    InvokeSuperRange,
    InvokeDirectRange,
    InvokeStaticRange,
    InvokeInterfaceRange,
    InvokePolymorphic,
    InvokePolymorphicRange,
    InvokeCustom,
    InvokeCustomRange,
    ConstMethodHandle,
    ConstMethodType,
    NegInt,
    NotInt,
    NegLong,
    NotLong,
    NegFloat,
    NegDouble,
    IntToLong,
    IntToFloat,
    IntToDouble,
    LongToInt,
    LongToFloat,
    LongToDouble,
    FloatToInt,
    FloatToLong,
    FloatToDouble,
    DoubleToInt,
    DoubleToLong,
    DoubleToFloat,
    IntToByte,
    IntToChar,
    IntToShort,
    AddInt,
    SubInt,
    MulInt,
    DivInt,
    RemInt,
    AndInt,
    OrInt,
    XorInt,
    ShlInt,
    ShrInt,
    UshrInt,
    AddLong,
    SubLong,
    MulLong,
    DivLong,
    RemLong,
    AndLong,
    OrLong,
    XorLong,
    ShlLong,
    ShrLong,
    UshrLong,
    AddFloat,
    SubFloat,
    MulFloat,
    DivFloat,
    RemFloat,
    AddDouble,
    SubDouble,
    MulDouble,
    DivDouble,
    RemDouble,
    AddInt2Addr,
    SubInt2Addr,
    MulInt2Addr,
    DivInt2Addr,
    RemInt2Addr,
    AndInt2Addr,
    OrInt2Addr,
    XorInt2Addr,
    ShlInt2Addr,
    ShrInt2Addr,
    UshrInt2Addr,
    AddLong2Addr,
    SubLong2Addr,
    MulLong2Addr,
    DivLong2Addr,
    RemLong2Addr,
    AndLong2Addr,
    OrLong2Addr,
    XorLong2Addr,
    ShlLong2Addr,
    ShrLong2Addr,
    UshrLong2Addr,
    AddFloat2Addr,
    SubFloat2Addr,
    MulFloat2Addr,
    DivFloat2Addr,
    RemFloat2Addr,
    AddDouble2Addr,
    SubDouble2Addr,
    MulDouble2Addr,
    DivDouble2Addr,
    RemDouble2Addr,
    AddIntLit16,
    RsubIntLit16,
    MulIntLit16,
    DivIntLit16,
    RemIntLit16,
    AndIntLit16,
    OrIntLit16,
    XorIntLit16,
    AddIntLit8,
    RsubIntLit8,
    MulIntLit8,
    DivIntLit8,
    RemIntLit8,
    AndIntLit8,
    OrIntLit8,
    XorIntLit8,
    ShlIntLit8,
    ShrIntLit8,
    UshrIntLit8,
    PackedSwitchPayload,
    SparseSwitchPayload,
    FillArrayDataPayload,
    RawInstruction,
}

impl OpcodeMatcher {
    fn matches(&self, insn: &Instruction) -> bool {
        matches!(
            (self, insn),
            (Self::Nop, Instruction::Nop)
                | (Self::Move, Instruction::Move { .. })
                | (Self::MoveFrom16, Instruction::MoveFrom16 { .. })
                | (Self::Move16, Instruction::Move16 { .. })
                | (Self::MoveWide, Instruction::MoveWide { .. })
                | (Self::MoveWideFrom16, Instruction::MoveWideFrom16 { .. })
                | (Self::MoveWide16, Instruction::MoveWide16 { .. })
                | (Self::MoveObject, Instruction::MoveObject { .. })
                | (Self::MoveObjectFrom16, Instruction::MoveObjectFrom16 { .. })
                | (Self::MoveObject16, Instruction::MoveObject16 { .. })
                | (Self::MoveResult, Instruction::MoveResult { .. })
                | (Self::MoveResultWide, Instruction::MoveResultWide { .. })
                | (Self::MoveResultObject, Instruction::MoveResultObject { .. })
                | (Self::MoveException, Instruction::MoveException { .. })
                | (Self::ReturnVoid, Instruction::ReturnVoid)
                | (Self::Return, Instruction::Return { .. })
                | (Self::ReturnWide, Instruction::ReturnWide { .. })
                | (Self::ReturnObject, Instruction::ReturnObject { .. })
                | (Self::Const4, Instruction::Const4 { .. })
                | (Self::Const16, Instruction::Const16 { .. })
                | (Self::Const, Instruction::Const { .. })
                | (Self::ConstHigh16, Instruction::ConstHigh16 { .. })
                | (Self::ConstWide16, Instruction::ConstWide16 { .. })
                | (Self::ConstWide32, Instruction::ConstWide32 { .. })
                | (Self::ConstWide, Instruction::ConstWide { .. })
                | (Self::ConstWideHigh16, Instruction::ConstWideHigh16 { .. })
                | (Self::ConstString, Instruction::ConstString { .. })
                | (Self::ConstStringJumbo, Instruction::ConstStringJumbo { .. })
                | (Self::ConstClass, Instruction::ConstClass { .. })
                | (Self::MonitorEnter, Instruction::MonitorEnter { .. })
                | (Self::MonitorExit, Instruction::MonitorExit { .. })
                | (Self::CheckCast, Instruction::CheckCast { .. })
                | (Self::InstanceOf, Instruction::InstanceOf { .. })
                | (Self::ArrayLength, Instruction::ArrayLength { .. })
                | (Self::NewInstance, Instruction::NewInstance { .. })
                | (Self::NewArray, Instruction::NewArray { .. })
                | (Self::FilledNewArray, Instruction::FilledNewArray { .. })
                | (
                    Self::FilledNewArrayRange,
                    Instruction::FilledNewArrayRange { .. }
                )
                | (Self::FillArrayData, Instruction::FillArrayData { .. })
                | (Self::Throw, Instruction::Throw { .. })
                | (Self::Goto, Instruction::Goto { .. })
                | (Self::Goto16, Instruction::Goto16 { .. })
                | (Self::Goto32, Instruction::Goto32 { .. })
                | (Self::PackedSwitch, Instruction::PackedSwitch { .. })
                | (Self::SparseSwitch, Instruction::SparseSwitch { .. })
                | (Self::CmpLFloat, Instruction::CmpLFloat { .. })
                | (Self::CmpGFloat, Instruction::CmpGFloat { .. })
                | (Self::CmpLDouble, Instruction::CmpLDouble { .. })
                | (Self::CmpGDouble, Instruction::CmpGDouble { .. })
                | (Self::CmpLong, Instruction::CmpLong { .. })
                | (Self::IfEq, Instruction::IfEq { .. })
                | (Self::IfNe, Instruction::IfNe { .. })
                | (Self::IfLt, Instruction::IfLt { .. })
                | (Self::IfGe, Instruction::IfGe { .. })
                | (Self::IfGt, Instruction::IfGt { .. })
                | (Self::IfLe, Instruction::IfLe { .. })
                | (Self::IfEqz, Instruction::IfEqz { .. })
                | (Self::IfNez, Instruction::IfNez { .. })
                | (Self::IfLtz, Instruction::IfLtz { .. })
                | (Self::IfGez, Instruction::IfGez { .. })
                | (Self::IfGtz, Instruction::IfGtz { .. })
                | (Self::IfLez, Instruction::IfLez { .. })
                | (Self::Aget, Instruction::Aget { .. })
                | (Self::AgetWide, Instruction::AgetWide { .. })
                | (Self::AgetObject, Instruction::AgetObject { .. })
                | (Self::AgetBoolean, Instruction::AgetBoolean { .. })
                | (Self::AgetByte, Instruction::AgetByte { .. })
                | (Self::AgetChar, Instruction::AgetChar { .. })
                | (Self::AgetShort, Instruction::AgetShort { .. })
                | (Self::Aput, Instruction::Aput { .. })
                | (Self::AputWide, Instruction::AputWide { .. })
                | (Self::AputObject, Instruction::AputObject { .. })
                | (Self::AputBoolean, Instruction::AputBoolean { .. })
                | (Self::AputByte, Instruction::AputByte { .. })
                | (Self::AputChar, Instruction::AputChar { .. })
                | (Self::AputShort, Instruction::AputShort { .. })
                | (Self::Iget, Instruction::Iget { .. })
                | (Self::IgetWide, Instruction::IgetWide { .. })
                | (Self::IgetObject, Instruction::IgetObject { .. })
                | (Self::IgetBoolean, Instruction::IgetBoolean { .. })
                | (Self::IgetByte, Instruction::IgetByte { .. })
                | (Self::IgetChar, Instruction::IgetChar { .. })
                | (Self::IgetShort, Instruction::IgetShort { .. })
                | (Self::Iput, Instruction::Iput { .. })
                | (Self::IputWide, Instruction::IputWide { .. })
                | (Self::IputObject, Instruction::IputObject { .. })
                | (Self::IputBoolean, Instruction::IputBoolean { .. })
                | (Self::IputByte, Instruction::IputByte { .. })
                | (Self::IputChar, Instruction::IputChar { .. })
                | (Self::IputShort, Instruction::IputShort { .. })
                | (Self::Sget, Instruction::Sget { .. })
                | (Self::SgetWide, Instruction::SgetWide { .. })
                | (Self::SgetObject, Instruction::SgetObject { .. })
                | (Self::SgetBoolean, Instruction::SgetBoolean { .. })
                | (Self::SgetByte, Instruction::SgetByte { .. })
                | (Self::SgetChar, Instruction::SgetChar { .. })
                | (Self::SgetShort, Instruction::SgetShort { .. })
                | (Self::Sput, Instruction::Sput { .. })
                | (Self::SputWide, Instruction::SputWide { .. })
                | (Self::SputObject, Instruction::SputObject { .. })
                | (Self::SputBoolean, Instruction::SputBoolean { .. })
                | (Self::SputByte, Instruction::SputByte { .. })
                | (Self::SputChar, Instruction::SputChar { .. })
                | (Self::SputShort, Instruction::SputShort { .. })
                | (Self::InvokeVirtual, Instruction::InvokeVirtual { .. })
                | (Self::InvokeSuper, Instruction::InvokeSuper { .. })
                | (Self::InvokeDirect, Instruction::InvokeDirect { .. })
                | (Self::InvokeStatic, Instruction::InvokeStatic { .. })
                | (Self::InvokeInterface, Instruction::InvokeInterface { .. })
                | (
                    Self::InvokeVirtualRange,
                    Instruction::InvokeVirtualRange { .. }
                )
                | (Self::InvokeSuperRange, Instruction::InvokeSuperRange { .. })
                | (
                    Self::InvokeDirectRange,
                    Instruction::InvokeDirectRange { .. }
                )
                | (
                    Self::InvokeStaticRange,
                    Instruction::InvokeStaticRange { .. }
                )
                | (
                    Self::InvokeInterfaceRange,
                    Instruction::InvokeInterfaceRange { .. }
                )
                | (
                    Self::InvokePolymorphic,
                    Instruction::InvokePolymorphic { .. }
                )
                | (
                    Self::InvokePolymorphicRange,
                    Instruction::InvokePolymorphicRange { .. }
                )
                | (Self::InvokeCustom, Instruction::InvokeCustom { .. })
                | (
                    Self::InvokeCustomRange,
                    Instruction::InvokeCustomRange { .. }
                )
                | (
                    Self::ConstMethodHandle,
                    Instruction::ConstMethodHandle { .. }
                )
                | (Self::ConstMethodType, Instruction::ConstMethodType { .. })
                | (Self::NegInt, Instruction::NegInt { .. })
                | (Self::NotInt, Instruction::NotInt { .. })
                | (Self::NegLong, Instruction::NegLong { .. })
                | (Self::NotLong, Instruction::NotLong { .. })
                | (Self::NegFloat, Instruction::NegFloat { .. })
                | (Self::NegDouble, Instruction::NegDouble { .. })
                | (Self::IntToLong, Instruction::IntToLong { .. })
                | (Self::IntToFloat, Instruction::IntToFloat { .. })
                | (Self::IntToDouble, Instruction::IntToDouble { .. })
                | (Self::LongToInt, Instruction::LongToInt { .. })
                | (Self::LongToFloat, Instruction::LongToFloat { .. })
                | (Self::LongToDouble, Instruction::LongToDouble { .. })
                | (Self::FloatToInt, Instruction::FloatToInt { .. })
                | (Self::FloatToLong, Instruction::FloatToLong { .. })
                | (Self::FloatToDouble, Instruction::FloatToDouble { .. })
                | (Self::DoubleToInt, Instruction::DoubleToInt { .. })
                | (Self::DoubleToLong, Instruction::DoubleToLong { .. })
                | (Self::DoubleToFloat, Instruction::DoubleToFloat { .. })
                | (Self::IntToByte, Instruction::IntToByte { .. })
                | (Self::IntToChar, Instruction::IntToChar { .. })
                | (Self::IntToShort, Instruction::IntToShort { .. })
                | (Self::AddInt, Instruction::AddInt { .. })
                | (Self::SubInt, Instruction::SubInt { .. })
                | (Self::MulInt, Instruction::MulInt { .. })
                | (Self::DivInt, Instruction::DivInt { .. })
                | (Self::RemInt, Instruction::RemInt { .. })
                | (Self::AndInt, Instruction::AndInt { .. })
                | (Self::OrInt, Instruction::OrInt { .. })
                | (Self::XorInt, Instruction::XorInt { .. })
                | (Self::ShlInt, Instruction::ShlInt { .. })
                | (Self::ShrInt, Instruction::ShrInt { .. })
                | (Self::UshrInt, Instruction::UshrInt { .. })
                | (Self::AddLong, Instruction::AddLong { .. })
                | (Self::SubLong, Instruction::SubLong { .. })
                | (Self::MulLong, Instruction::MulLong { .. })
                | (Self::DivLong, Instruction::DivLong { .. })
                | (Self::RemLong, Instruction::RemLong { .. })
                | (Self::AndLong, Instruction::AndLong { .. })
                | (Self::OrLong, Instruction::OrLong { .. })
                | (Self::XorLong, Instruction::XorLong { .. })
                | (Self::ShlLong, Instruction::ShlLong { .. })
                | (Self::ShrLong, Instruction::ShrLong { .. })
                | (Self::UshrLong, Instruction::UshrLong { .. })
                | (Self::AddFloat, Instruction::AddFloat { .. })
                | (Self::SubFloat, Instruction::SubFloat { .. })
                | (Self::MulFloat, Instruction::MulFloat { .. })
                | (Self::DivFloat, Instruction::DivFloat { .. })
                | (Self::RemFloat, Instruction::RemFloat { .. })
                | (Self::AddDouble, Instruction::AddDouble { .. })
                | (Self::SubDouble, Instruction::SubDouble { .. })
                | (Self::MulDouble, Instruction::MulDouble { .. })
                | (Self::DivDouble, Instruction::DivDouble { .. })
                | (Self::RemDouble, Instruction::RemDouble { .. })
                | (Self::AddInt2Addr, Instruction::AddInt2Addr { .. })
                | (Self::SubInt2Addr, Instruction::SubInt2Addr { .. })
                | (Self::MulInt2Addr, Instruction::MulInt2Addr { .. })
                | (Self::DivInt2Addr, Instruction::DivInt2Addr { .. })
                | (Self::RemInt2Addr, Instruction::RemInt2Addr { .. })
                | (Self::AndInt2Addr, Instruction::AndInt2Addr { .. })
                | (Self::OrInt2Addr, Instruction::OrInt2Addr { .. })
                | (Self::XorInt2Addr, Instruction::XorInt2Addr { .. })
                | (Self::ShlInt2Addr, Instruction::ShlInt2Addr { .. })
                | (Self::ShrInt2Addr, Instruction::ShrInt2Addr { .. })
                | (Self::UshrInt2Addr, Instruction::UshrInt2Addr { .. })
                | (Self::AddLong2Addr, Instruction::AddLong2Addr { .. })
                | (Self::SubLong2Addr, Instruction::SubLong2Addr { .. })
                | (Self::MulLong2Addr, Instruction::MulLong2Addr { .. })
                | (Self::DivLong2Addr, Instruction::DivLong2Addr { .. })
                | (Self::RemLong2Addr, Instruction::RemLong2Addr { .. })
                | (Self::AndLong2Addr, Instruction::AndLong2Addr { .. })
                | (Self::OrLong2Addr, Instruction::OrLong2Addr { .. })
                | (Self::XorLong2Addr, Instruction::XorLong2Addr { .. })
                | (Self::ShlLong2Addr, Instruction::ShlLong2Addr { .. })
                | (Self::ShrLong2Addr, Instruction::ShrLong2Addr { .. })
                | (Self::UshrLong2Addr, Instruction::UshrLong2Addr { .. })
                | (Self::AddFloat2Addr, Instruction::AddFloat2Addr { .. })
                | (Self::SubFloat2Addr, Instruction::SubFloat2Addr { .. })
                | (Self::MulFloat2Addr, Instruction::MulFloat2Addr { .. })
                | (Self::DivFloat2Addr, Instruction::DivFloat2Addr { .. })
                | (Self::RemFloat2Addr, Instruction::RemFloat2Addr { .. })
                | (Self::AddDouble2Addr, Instruction::AddDouble2Addr { .. })
                | (Self::SubDouble2Addr, Instruction::SubDouble2Addr { .. })
                | (Self::MulDouble2Addr, Instruction::MulDouble2Addr { .. })
                | (Self::DivDouble2Addr, Instruction::DivDouble2Addr { .. })
                | (Self::RemDouble2Addr, Instruction::RemDouble2Addr { .. })
                | (Self::AddIntLit16, Instruction::AddIntLit16 { .. })
                | (Self::RsubIntLit16, Instruction::RsubIntLit16 { .. })
                | (Self::MulIntLit16, Instruction::MulIntLit16 { .. })
                | (Self::DivIntLit16, Instruction::DivIntLit16 { .. })
                | (Self::RemIntLit16, Instruction::RemIntLit16 { .. })
                | (Self::AndIntLit16, Instruction::AndIntLit16 { .. })
                | (Self::OrIntLit16, Instruction::OrIntLit16 { .. })
                | (Self::XorIntLit16, Instruction::XorIntLit16 { .. })
                | (Self::AddIntLit8, Instruction::AddIntLit8 { .. })
                | (Self::RsubIntLit8, Instruction::RsubIntLit8 { .. })
                | (Self::MulIntLit8, Instruction::MulIntLit8 { .. })
                | (Self::DivIntLit8, Instruction::DivIntLit8 { .. })
                | (Self::RemIntLit8, Instruction::RemIntLit8 { .. })
                | (Self::AndIntLit8, Instruction::AndIntLit8 { .. })
                | (Self::OrIntLit8, Instruction::OrIntLit8 { .. })
                | (Self::XorIntLit8, Instruction::XorIntLit8 { .. })
                | (Self::ShlIntLit8, Instruction::ShlIntLit8 { .. })
                | (Self::ShrIntLit8, Instruction::ShrIntLit8 { .. })
                | (Self::UshrIntLit8, Instruction::UshrIntLit8 { .. })
                | (
                    Self::PackedSwitchPayload,
                    Instruction::PackedSwitchPayload { .. }
                )
                | (
                    Self::SparseSwitchPayload,
                    Instruction::SparseSwitchPayload { .. }
                )
                | (
                    Self::FillArrayDataPayload,
                    Instruction::FillArrayDataPayload { .. }
                )
                | (Self::RawInstruction, Instruction::RawInstruction { .. })
        )
    }
}

pub(super) fn matches_pattern(
    instructions: &[Instruction],
    pattern: &[InstructionPattern],
) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if instructions.len() < pattern.len() {
        return false;
    }
    'outer: for start in 0..=instructions.len() - pattern.len() {
        for (i, pat) in pattern.iter().enumerate() {
            match pat {
                InstructionPattern::Any => {}
                InstructionPattern::Opcode(matcher) => {
                    if !matcher.matches(&instructions[start + i]) {
                        continue 'outer;
                    }
                }
            }
        }
        return true;
    }
    false
}
