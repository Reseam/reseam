// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

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

#[data]
#[derive(Debug, Clone)]
pub struct ScratchSpan {
    pub instruction_index: u32,
    pub registers: Vec<u16>,
}

#[data]
#[derive(Debug, Clone)]
pub struct MethodEdit {
    pub register_shift: u16,
    pub starts: Vec<u32>,
    pub instructions: Vec<u32>,
    pub ends: Vec<u32>,
}
