// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::data;

use super::types::{CallSiteRef, FieldRef, HandleRef, MethodRef};

#[data]
#[derive(Debug, Clone, Copy)]
pub struct SimpleInsn {
    pub opcode: u16,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct Reg1Insn {
    pub opcode: u16,
    pub reg_a: u16,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct Reg2Insn {
    pub opcode: u16,
    pub reg_a: u16,
    pub reg_b: u16,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct Reg3Insn {
    pub opcode: u16,
    pub reg_a: u16,
    pub reg_b: u16,
    pub reg_c: u16,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct RegLiteralInsn {
    pub opcode: u16,
    pub reg_a: u16,
    pub reg_b: u16,
    pub literal: i64,
}

#[data]
#[derive(Debug, Clone)]
pub struct RegStringInsn {
    pub opcode: u16,
    pub reg_a: u16,
    pub value: String,
}

#[data]
#[derive(Debug, Clone)]
pub struct RegTypeInsn {
    pub opcode: u16,
    pub reg_a: u16,
    pub reg_b: u16,
    pub type_descriptor: String,
}

#[data]
#[derive(Debug, Clone)]
pub struct RegFieldInsn {
    pub opcode: u16,
    pub reg_a: u16,
    pub reg_b: u16,
    pub field: FieldRef,
}

#[data]
#[derive(Debug, Clone)]
pub struct InvokeInsn {
    pub opcode: u16,
    pub registers: Vec<u16>,
    pub method: MethodRef,
}

#[data]
#[derive(Debug, Clone)]
pub struct InvokeRangeInsn {
    pub opcode: u16,
    pub start_reg: u16,
    pub reg_count: u16,
    pub method: MethodRef,
}

#[data]
#[derive(Debug, Clone)]
pub struct PolymorphicInsn {
    pub opcode: u16,
    pub method: MethodRef,
    pub proto: String,
    pub registers: Vec<u16>,
}

#[data]
#[derive(Debug, Clone)]
pub struct PolymorphicRangeInsn {
    pub opcode: u16,
    pub method: MethodRef,
    pub proto: String,
    pub start_reg: u16,
    pub reg_count: u16,
}

#[data]
#[derive(Debug, Clone)]
pub struct CustomInsn {
    pub opcode: u16,
    pub call_site: CallSiteRef,
    pub registers: Vec<u16>,
}

#[data]
#[derive(Debug, Clone)]
pub struct CustomRangeInsn {
    pub opcode: u16,
    pub call_site: CallSiteRef,
    pub start_reg: u16,
    pub reg_count: u16,
}

#[data]
#[derive(Debug, Clone)]
pub struct RegHandleInsn {
    pub opcode: u16,
    pub reg_a: u16,
    pub handle: HandleRef,
}

#[data]
#[derive(Debug, Clone)]
pub struct RegProtoInsn {
    pub opcode: u16,
    pub reg_a: u16,
    pub proto: String,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct Branch0Insn {
    pub opcode: u16,
    pub offset: i32,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct BranchInsn {
    pub opcode: u16,
    pub reg_a: u16,
    pub offset: i32,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct Branch2Insn {
    pub opcode: u16,
    pub reg_a: u16,
    pub reg_b: u16,
    pub offset: i32,
}

#[data]
#[derive(Debug, Clone)]
pub struct FilledArrayInsn {
    pub opcode: u16,
    pub registers: Vec<u16>,
    pub type_descriptor: String,
}

#[data]
#[derive(Debug, Clone)]
pub struct FilledArrayRangeInsn {
    pub opcode: u16,
    pub start_reg: u16,
    pub reg_count: u16,
    pub type_descriptor: String,
}

#[data]
#[derive(Debug, Clone)]
pub struct PackedSwitchInsn {
    pub first_key: i32,
    pub targets: Vec<i32>,
}

#[data]
#[derive(Debug, Clone)]
pub struct SparseSwitchInsn {
    pub keys: Vec<i32>,
    pub targets: Vec<i32>,
}

#[data]
#[derive(Debug, Clone)]
pub struct FillArrayInsn {
    pub element_width: u16,
    pub data: Vec<u8>,
}

#[data]
#[derive(Debug, Clone)]
pub enum Instruction {
    Simple(SimpleInsn),
    Reg1(Reg1Insn),
    Reg2(Reg2Insn),
    Reg3(Reg3Insn),
    RegLiteral(RegLiteralInsn),
    RegString(RegStringInsn),
    RegType(RegTypeInsn),
    RegField(RegFieldInsn),
    Invoke(InvokeInsn),
    InvokeRange(InvokeRangeInsn),
    Polymorphic(PolymorphicInsn),
    PolymorphicRange(PolymorphicRangeInsn),
    Custom(CustomInsn),
    CustomRange(CustomRangeInsn),
    RegHandle(RegHandleInsn),
    RegProto(RegProtoInsn),
    Branch0(Branch0Insn),
    Branch(BranchInsn),
    Branch2(Branch2Insn),
    FilledArray(FilledArrayInsn),
    FilledArrayRange(FilledArrayRangeInsn),
    PackedSwitchData(PackedSwitchInsn),
    SparseSwitchData(SparseSwitchInsn),
    FillArrayData(FillArrayInsn),
    Raw(Vec<u8>),
}

/// How an edit moved a method's code: where each original instruction went and, when the frame
/// grew, by how many registers it moved those at or above `register_base`.
#[data]
#[derive(Debug, Clone)]
pub struct MethodEdit {
    pub register_base: u16,
    pub register_shift: u16,
    pub starts: Vec<u32>,
    pub instructions: Vec<u32>,
    pub ends: Vec<u32>,
}

impl Instruction {
    /// The opcode of a structured instruction; `None` for payloads and raw code.
    pub(in crate::bridge) fn opcode(&self) -> Option<u16> {
        Some(match self {
            Instruction::Simple(value) => value.opcode,
            Instruction::Reg1(value) => value.opcode,
            Instruction::Reg2(value) => value.opcode,
            Instruction::Reg3(value) => value.opcode,
            Instruction::RegLiteral(value) => value.opcode,
            Instruction::RegString(value) => value.opcode,
            Instruction::RegType(value) => value.opcode,
            Instruction::RegField(value) => value.opcode,
            Instruction::Invoke(value) => value.opcode,
            Instruction::InvokeRange(value) => value.opcode,
            Instruction::Polymorphic(value) => value.opcode,
            Instruction::PolymorphicRange(value) => value.opcode,
            Instruction::Custom(value) => value.opcode,
            Instruction::CustomRange(value) => value.opcode,
            Instruction::RegHandle(value) => value.opcode,
            Instruction::RegProto(value) => value.opcode,
            Instruction::Branch0(value) => value.opcode,
            Instruction::Branch(value) => value.opcode,
            Instruction::Branch2(value) => value.opcode,
            Instruction::FilledArray(value) => value.opcode,
            Instruction::FilledArrayRange(value) => value.opcode,
            _ => return None,
        })
    }
}

/// Where an operand of emitted code lives.
#[data]
#[derive(Debug, Clone, Copy)]
pub enum CodeRegister {
    /// A value the code computes, in a register the engine chooses.
    Temporary(u32),
    /// A register of the method as it is now.
    Fixed(u16),
    /// The incoming register word at this offset, as the method was entered.
    Parameter(u16),
    /// The value the return the code runs before returns.
    Returned,
}

#[data]
#[derive(Debug, Clone)]
pub struct CodeOperand {
    pub register: CodeRegister,
    pub type_descriptor: String,
}

/// An instruction of emitted code. Its own registers are placeholders: its register operands,
/// in encoding order, are `operands`, and a call's are its arguments. A branch goes to the
/// label `target`.
#[data]
#[derive(Debug, Clone)]
pub struct CodeInstruction {
    pub template: Instruction,
    pub operands: Vec<CodeOperand>,
    #[boltffi::default(None)]
    pub target: Option<u32>,
}

#[data]
#[derive(Debug, Clone)]
#[cfg_attr(
    target_pointer_width = "64",
    expect(
        clippy::large_enum_variant,
        reason = "BoltFFI cannot carry a boxed payload across the bridge"
    )
)]
pub enum CodeStep {
    Label(u32),
    Instruction(CodeInstruction),
}

/// Where emitted code goes in its method.
#[data]
#[derive(Debug, Clone, Copy)]
pub enum CodePlacement {
    /// Before the instruction at this index; branches to it skip the code.
    Before(u32),
    /// Before the instruction at this index, on every path to it.
    OnEveryPath(u32),
    /// Before each return, reading parameters as the method was entered.
    BeforeReturns,
    /// As the whole body.
    Body,
}

#[data]
#[derive(Debug, Clone)]
pub enum CodeEdit {
    Grow(MethodEdit),
    Insert(MethodEdit),
}
