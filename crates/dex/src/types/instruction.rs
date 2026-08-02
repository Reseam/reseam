// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::method_handle::{CallSiteIdx, MethodHandleIdx};
use super::{FieldIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

pub type U4 = u8;
pub type I4 = i8;

/// Register list for a 35c-form invocation: at most five args, stored inline
/// with no heap allocation. Derefs to `[u8]` so callers read it like a slice.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegList {
    regs: [u8; 5],
    len: u8,
}

impl RegList {
    pub const fn new() -> Self {
        Self {
            regs: [0; 5],
            len: 0,
        }
    }

    pub fn push(&mut self, reg: u8) {
        debug_assert!(
            (self.len as usize) < self.regs.len(),
            "a 35c invocation takes at most 5 register args"
        );
        self.regs[self.len as usize] = reg;
        self.len += 1;
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.regs[..self.len as usize]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.regs[..self.len as usize]
    }
}

impl core::ops::Deref for RegList {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl core::ops::DerefMut for RegList {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

impl FromIterator<u8> for RegList {
    fn from_iter<I: IntoIterator<Item = u8>>(iter: I) -> Self {
        let mut list = RegList::new();
        for reg in iter {
            list.push(reg);
        }
        list
    }
}

impl From<&[u8]> for RegList {
    fn from(slice: &[u8]) -> Self {
        slice.iter().copied().collect()
    }
}

impl<'a> IntoIterator for &'a RegList {
    type Item = &'a u8;
    type IntoIter = core::slice::Iter<'a, u8>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Payload of a `packed-switch`, boxed so the rare variant does not widen the
/// common `Instruction` cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedSwitchData {
    pub first_key: i32,
    pub targets: Vec<i32>,
}

/// Payload of a `sparse-switch`, boxed for the same reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseSwitchData {
    pub keys_and_targets: Vec<(i32, i32)>,
}

/// Payload of a `fill-array-data`, boxed for the same reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillArrayPayloadData {
    pub element_width: u16,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Instruction {
    Nop,
    Move {
        dest: U4,
        src: U4,
    },
    MoveFrom16 {
        dest: u8,
        src: u16,
    },
    Move16 {
        dest: u16,
        src: u16,
    },
    MoveWide {
        dest: U4,
        src: U4,
    },
    MoveWideFrom16 {
        dest: u8,
        src: u16,
    },
    MoveWide16 {
        dest: u16,
        src: u16,
    },
    MoveObject {
        dest: U4,
        src: U4,
    },
    MoveObjectFrom16 {
        dest: u8,
        src: u16,
    },
    MoveObject16 {
        dest: u16,
        src: u16,
    },
    MoveResult {
        dest: u8,
    },
    MoveResultWide {
        dest: u8,
    },
    MoveResultObject {
        dest: u8,
    },
    MoveException {
        dest: u8,
    },
    ReturnVoid,
    Return {
        src: u8,
    },
    ReturnWide {
        src: u8,
    },
    ReturnObject {
        src: u8,
    },
    Const4 {
        dest: U4,
        value: I4,
    },
    Const16 {
        dest: u8,
        value: i16,
    },
    Const {
        dest: u8,
        value: i32,
    },
    ConstHigh16 {
        dest: u8,
        value: i16,
    },
    ConstWide16 {
        dest: u8,
        value: i16,
    },
    ConstWide32 {
        dest: u8,
        value: i32,
    },
    ConstWide {
        dest: u8,
        value: i64,
    },
    ConstWideHigh16 {
        dest: u8,
        value: i16,
    },
    ConstString {
        dest: u8,
        string: StringIdx,
    },
    ConstStringJumbo {
        dest: u8,
        string: StringIdx,
    },
    ConstClass {
        dest: u8,
        type_: TypeIdx,
    },
    MonitorEnter {
        ref_: u8,
    },
    MonitorExit {
        ref_: u8,
    },
    CheckCast {
        ref_: u8,
        type_: TypeIdx,
    },
    InstanceOf {
        dest: U4,
        ref_: U4,
        type_: TypeIdx,
    },
    ArrayLength {
        dest: U4,
        array: U4,
    },
    NewInstance {
        dest: u8,
        type_: TypeIdx,
    },
    NewArray {
        dest: U4,
        size: U4,
        type_: TypeIdx,
    },
    FilledNewArray {
        type_: TypeIdx,
        args: RegList,
    },
    FilledNewArrayRange {
        type_: TypeIdx,
        first_reg: u16,
        count: u8,
    },
    FillArrayData {
        array: u8,
        payload_offset: i32,
    },
    Throw {
        exception: u8,
    },
    Goto {
        offset: i8,
    },
    Goto16 {
        offset: i16,
    },
    Goto32 {
        offset: i32,
    },
    PackedSwitch {
        test: u8,
        payload_offset: i32,
    },
    SparseSwitch {
        test: u8,
        payload_offset: i32,
    },
    CmpLFloat {
        dest: u8,
        a: u8,
        b: u8,
    },
    CmpGFloat {
        dest: u8,
        a: u8,
        b: u8,
    },
    CmpLDouble {
        dest: u8,
        a: u8,
        b: u8,
    },
    CmpGDouble {
        dest: u8,
        a: u8,
        b: u8,
    },
    CmpLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    IfEq {
        a: U4,
        b: U4,
        offset: i16,
    },
    IfNe {
        a: U4,
        b: U4,
        offset: i16,
    },
    IfLt {
        a: U4,
        b: U4,
        offset: i16,
    },
    IfGe {
        a: U4,
        b: U4,
        offset: i16,
    },
    IfGt {
        a: U4,
        b: U4,
        offset: i16,
    },
    IfLe {
        a: U4,
        b: U4,
        offset: i16,
    },
    IfEqz {
        a: u8,
        offset: i16,
    },
    IfNez {
        a: u8,
        offset: i16,
    },
    IfLtz {
        a: u8,
        offset: i16,
    },
    IfGez {
        a: u8,
        offset: i16,
    },
    IfGtz {
        a: u8,
        offset: i16,
    },
    IfLez {
        a: u8,
        offset: i16,
    },
    Aget {
        dest: u8,
        array: u8,
        index: u8,
    },
    AgetWide {
        dest: u8,
        array: u8,
        index: u8,
    },
    AgetObject {
        dest: u8,
        array: u8,
        index: u8,
    },
    AgetBoolean {
        dest: u8,
        array: u8,
        index: u8,
    },
    AgetByte {
        dest: u8,
        array: u8,
        index: u8,
    },
    AgetChar {
        dest: u8,
        array: u8,
        index: u8,
    },
    AgetShort {
        dest: u8,
        array: u8,
        index: u8,
    },
    Aput {
        src: u8,
        array: u8,
        index: u8,
    },
    AputWide {
        src: u8,
        array: u8,
        index: u8,
    },
    AputObject {
        src: u8,
        array: u8,
        index: u8,
    },
    AputBoolean {
        src: u8,
        array: u8,
        index: u8,
    },
    AputByte {
        src: u8,
        array: u8,
        index: u8,
    },
    AputChar {
        src: u8,
        array: u8,
        index: u8,
    },
    AputShort {
        src: u8,
        array: u8,
        index: u8,
    },
    Iget {
        dest: U4,
        obj: U4,
        field: FieldIdx,
    },
    IgetWide {
        dest: U4,
        obj: U4,
        field: FieldIdx,
    },
    IgetObject {
        dest: U4,
        obj: U4,
        field: FieldIdx,
    },
    IgetBoolean {
        dest: U4,
        obj: U4,
        field: FieldIdx,
    },
    IgetByte {
        dest: U4,
        obj: U4,
        field: FieldIdx,
    },
    IgetChar {
        dest: U4,
        obj: U4,
        field: FieldIdx,
    },
    IgetShort {
        dest: U4,
        obj: U4,
        field: FieldIdx,
    },
    Iput {
        src: U4,
        obj: U4,
        field: FieldIdx,
    },
    IputWide {
        src: U4,
        obj: U4,
        field: FieldIdx,
    },
    IputObject {
        src: U4,
        obj: U4,
        field: FieldIdx,
    },
    IputBoolean {
        src: U4,
        obj: U4,
        field: FieldIdx,
    },
    IputByte {
        src: U4,
        obj: U4,
        field: FieldIdx,
    },
    IputChar {
        src: U4,
        obj: U4,
        field: FieldIdx,
    },
    IputShort {
        src: U4,
        obj: U4,
        field: FieldIdx,
    },
    Sget {
        dest: u8,
        field: FieldIdx,
    },
    SgetWide {
        dest: u8,
        field: FieldIdx,
    },
    SgetObject {
        dest: u8,
        field: FieldIdx,
    },
    SgetBoolean {
        dest: u8,
        field: FieldIdx,
    },
    SgetByte {
        dest: u8,
        field: FieldIdx,
    },
    SgetChar {
        dest: u8,
        field: FieldIdx,
    },
    SgetShort {
        dest: u8,
        field: FieldIdx,
    },
    Sput {
        src: u8,
        field: FieldIdx,
    },
    SputWide {
        src: u8,
        field: FieldIdx,
    },
    SputObject {
        src: u8,
        field: FieldIdx,
    },
    SputBoolean {
        src: u8,
        field: FieldIdx,
    },
    SputByte {
        src: u8,
        field: FieldIdx,
    },
    SputChar {
        src: u8,
        field: FieldIdx,
    },
    SputShort {
        src: u8,
        field: FieldIdx,
    },
    InvokeVirtual {
        method: MethodIdx,
        args: RegList,
    },
    InvokeSuper {
        method: MethodIdx,
        args: RegList,
    },
    InvokeDirect {
        method: MethodIdx,
        args: RegList,
    },
    InvokeStatic {
        method: MethodIdx,
        args: RegList,
    },
    InvokeInterface {
        method: MethodIdx,
        args: RegList,
    },
    InvokeVirtualRange {
        method: MethodIdx,
        first_reg: u16,
        count: u8,
    },
    InvokeSuperRange {
        method: MethodIdx,
        first_reg: u16,
        count: u8,
    },
    InvokeDirectRange {
        method: MethodIdx,
        first_reg: u16,
        count: u8,
    },
    InvokeStaticRange {
        method: MethodIdx,
        first_reg: u16,
        count: u8,
    },
    InvokeInterfaceRange {
        method: MethodIdx,
        first_reg: u16,
        count: u8,
    },
    InvokePolymorphic {
        method: MethodIdx,
        proto: ProtoIdx,
        args: RegList,
    },
    InvokePolymorphicRange {
        method: MethodIdx,
        proto: ProtoIdx,
        first_reg: u16,
        count: u8,
    },
    InvokeCustom {
        call_site: CallSiteIdx,
        args: RegList,
    },
    InvokeCustomRange {
        call_site: CallSiteIdx,
        first_reg: u16,
        count: u8,
    },
    ConstMethodHandle {
        dest: u8,
        method_handle: MethodHandleIdx,
    },
    ConstMethodType {
        dest: u8,
        proto: ProtoIdx,
    },
    NegInt {
        dest: U4,
        src: U4,
    },
    NotInt {
        dest: U4,
        src: U4,
    },
    NegLong {
        dest: U4,
        src: U4,
    },
    NotLong {
        dest: U4,
        src: U4,
    },
    NegFloat {
        dest: U4,
        src: U4,
    },
    NegDouble {
        dest: U4,
        src: U4,
    },
    IntToLong {
        dest: U4,
        src: U4,
    },
    IntToFloat {
        dest: U4,
        src: U4,
    },
    IntToDouble {
        dest: U4,
        src: U4,
    },
    LongToInt {
        dest: U4,
        src: U4,
    },
    LongToFloat {
        dest: U4,
        src: U4,
    },
    LongToDouble {
        dest: U4,
        src: U4,
    },
    FloatToInt {
        dest: U4,
        src: U4,
    },
    FloatToLong {
        dest: U4,
        src: U4,
    },
    FloatToDouble {
        dest: U4,
        src: U4,
    },
    DoubleToInt {
        dest: U4,
        src: U4,
    },
    DoubleToLong {
        dest: U4,
        src: U4,
    },
    DoubleToFloat {
        dest: U4,
        src: U4,
    },
    IntToByte {
        dest: U4,
        src: U4,
    },
    IntToChar {
        dest: U4,
        src: U4,
    },
    IntToShort {
        dest: U4,
        src: U4,
    },
    AddInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    SubInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    MulInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    DivInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    RemInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    AndInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    OrInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    XorInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    ShlInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    ShrInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    UshrInt {
        dest: u8,
        a: u8,
        b: u8,
    },
    AddLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    SubLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    MulLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    DivLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    RemLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    AndLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    OrLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    XorLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    ShlLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    ShrLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    UshrLong {
        dest: u8,
        a: u8,
        b: u8,
    },
    AddFloat {
        dest: u8,
        a: u8,
        b: u8,
    },
    SubFloat {
        dest: u8,
        a: u8,
        b: u8,
    },
    MulFloat {
        dest: u8,
        a: u8,
        b: u8,
    },
    DivFloat {
        dest: u8,
        a: u8,
        b: u8,
    },
    RemFloat {
        dest: u8,
        a: u8,
        b: u8,
    },
    AddDouble {
        dest: u8,
        a: u8,
        b: u8,
    },
    SubDouble {
        dest: u8,
        a: u8,
        b: u8,
    },
    MulDouble {
        dest: u8,
        a: u8,
        b: u8,
    },
    DivDouble {
        dest: u8,
        a: u8,
        b: u8,
    },
    RemDouble {
        dest: u8,
        a: u8,
        b: u8,
    },
    AddInt2Addr {
        dest_a: U4,
        b: U4,
    },
    SubInt2Addr {
        dest_a: U4,
        b: U4,
    },
    MulInt2Addr {
        dest_a: U4,
        b: U4,
    },
    DivInt2Addr {
        dest_a: U4,
        b: U4,
    },
    RemInt2Addr {
        dest_a: U4,
        b: U4,
    },
    AndInt2Addr {
        dest_a: U4,
        b: U4,
    },
    OrInt2Addr {
        dest_a: U4,
        b: U4,
    },
    XorInt2Addr {
        dest_a: U4,
        b: U4,
    },
    ShlInt2Addr {
        dest_a: U4,
        b: U4,
    },
    ShrInt2Addr {
        dest_a: U4,
        b: U4,
    },
    UshrInt2Addr {
        dest_a: U4,
        b: U4,
    },
    AddLong2Addr {
        dest_a: U4,
        b: U4,
    },
    SubLong2Addr {
        dest_a: U4,
        b: U4,
    },
    MulLong2Addr {
        dest_a: U4,
        b: U4,
    },
    DivLong2Addr {
        dest_a: U4,
        b: U4,
    },
    RemLong2Addr {
        dest_a: U4,
        b: U4,
    },
    AndLong2Addr {
        dest_a: U4,
        b: U4,
    },
    OrLong2Addr {
        dest_a: U4,
        b: U4,
    },
    XorLong2Addr {
        dest_a: U4,
        b: U4,
    },
    ShlLong2Addr {
        dest_a: U4,
        b: U4,
    },
    ShrLong2Addr {
        dest_a: U4,
        b: U4,
    },
    UshrLong2Addr {
        dest_a: U4,
        b: U4,
    },
    AddFloat2Addr {
        dest_a: U4,
        b: U4,
    },
    SubFloat2Addr {
        dest_a: U4,
        b: U4,
    },
    MulFloat2Addr {
        dest_a: U4,
        b: U4,
    },
    DivFloat2Addr {
        dest_a: U4,
        b: U4,
    },
    RemFloat2Addr {
        dest_a: U4,
        b: U4,
    },
    AddDouble2Addr {
        dest_a: U4,
        b: U4,
    },
    SubDouble2Addr {
        dest_a: U4,
        b: U4,
    },
    MulDouble2Addr {
        dest_a: U4,
        b: U4,
    },
    DivDouble2Addr {
        dest_a: U4,
        b: U4,
    },
    RemDouble2Addr {
        dest_a: U4,
        b: U4,
    },
    AddIntLit16 {
        dest: U4,
        src: U4,
        literal: i16,
    },
    RsubIntLit16 {
        dest: U4,
        src: U4,
        literal: i16,
    },
    MulIntLit16 {
        dest: U4,
        src: U4,
        literal: i16,
    },
    DivIntLit16 {
        dest: U4,
        src: U4,
        literal: i16,
    },
    RemIntLit16 {
        dest: U4,
        src: U4,
        literal: i16,
    },
    AndIntLit16 {
        dest: U4,
        src: U4,
        literal: i16,
    },
    OrIntLit16 {
        dest: U4,
        src: U4,
        literal: i16,
    },
    XorIntLit16 {
        dest: U4,
        src: U4,
        literal: i16,
    },
    AddIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    RsubIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    MulIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    DivIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    RemIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    AndIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    OrIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    XorIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    ShlIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    ShrIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    UshrIntLit8 {
        dest: u8,
        src: u8,
        literal: i8,
    },
    PackedSwitchPayload(Box<PackedSwitchData>),
    SparseSwitchPayload(Box<SparseSwitchData>),
    FillArrayDataPayload(Box<FillArrayPayloadData>),
    RawInstruction {
        code_units: Box<[u16]>,
    },
}
