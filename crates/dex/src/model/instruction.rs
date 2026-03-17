use super::call_site::CallSiteIdx;
use super::field::FieldIdx;
use super::method::MethodIdx;
use super::method_handle::MethodHandleIdx;
use super::proto::ProtoIdx;
use super::string::StringIdx;
use super::types::TypeIdx;
use smallvec::SmallVec;

pub type U4 = u8;
pub type I4 = i8;

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
        args: SmallVec<[u8; 5]>,
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
        args: SmallVec<[u8; 5]>,
    },
    InvokeSuper {
        method: MethodIdx,
        args: SmallVec<[u8; 5]>,
    },
    InvokeDirect {
        method: MethodIdx,
        args: SmallVec<[u8; 5]>,
    },
    InvokeStatic {
        method: MethodIdx,
        args: SmallVec<[u8; 5]>,
    },
    InvokeInterface {
        method: MethodIdx,
        args: SmallVec<[u8; 5]>,
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
        args: SmallVec<[u8; 5]>,
    },
    InvokePolymorphicRange {
        method: MethodIdx,
        proto: ProtoIdx,
        first_reg: u16,
        count: u8,
    },
    InvokeCustom {
        call_site: CallSiteIdx,
        args: SmallVec<[u8; 5]>,
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

    PackedSwitchPayload {
        first_key: i32,
        targets: Vec<i32>,
    },
    SparseSwitchPayload {
        keys_and_targets: Vec<(i32, i32)>,
    },
    FillArrayDataPayload {
        element_width: u16,
        data: Vec<u8>,
    },

    RawInstruction {
        code_units: SmallVec<[u16; 5]>,
    },
}

impl Instruction {
    /// Size in 16-bit code units
    pub fn code_units(&self) -> u32 {
        match self {
            // Format 10x, 12x, 11n, 11x, 10t
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

            // Format 20t, 22x, 21t, 21s, 21h, 21c, 23x, 22b, 22t, 22s, 22c
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

            // Format 30t, 32x, 31i, 31t, 31c, 35c, 3rc
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

            // Format 45cc, 4rcc
            Self::InvokePolymorphic { .. } | Self::InvokePolymorphicRange { .. } => 4,

            // Format 51l
            Self::ConstWide { .. } => 5,

            // Payloads: sizes in u16 code units
            // packed-switch: ident(1) + size(1) + first_key(2) + targets(size*2)
            Self::PackedSwitchPayload { targets, .. } => (1 + 1 + 2 + targets.len() * 2) as u32,
            // sparse-switch: ident(1) + size(1) + keys(size*2) + targets(size*2)
            Self::SparseSwitchPayload {
                keys_and_targets, ..
            } => (1 + 1 + keys_and_targets.len() * 4) as u32,
            // fill-array-data: ident(1) + element_width(1) + size(2) + data(ceil to u16)
            Self::FillArrayDataPayload { data, .. } => (4 + data.len().div_ceil(2)) as u32,

            Self::RawInstruction { code_units } => code_units.len() as u32,
        }
    }

    /// Returns the Dalvik opcode for this instruction.
    ///
    /// For payload pseudo-instructions the ident word is returned
    /// (0x0100 packed-switch, 0x0200 sparse-switch, 0x0300 fill-array-data).
    /// Returns `None` for `RawInstruction` since it has no fixed opcode.
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

    pub fn method_ref(&self) -> Option<MethodIdx> {
        match self {
            Self::InvokeVirtual { method, .. }
            | Self::InvokeSuper { method, .. }
            | Self::InvokeDirect { method, .. }
            | Self::InvokeStatic { method, .. }
            | Self::InvokeInterface { method, .. }
            | Self::InvokeVirtualRange { method, .. }
            | Self::InvokeSuperRange { method, .. }
            | Self::InvokeDirectRange { method, .. }
            | Self::InvokeStaticRange { method, .. }
            | Self::InvokeInterfaceRange { method, .. }
            | Self::InvokePolymorphic { method, .. }
            | Self::InvokePolymorphicRange { method, .. } => Some(*method),
            _ => None,
        }
    }

    pub fn field_ref(&self) -> Option<FieldIdx> {
        match self {
            Self::Iget { field, .. }
            | Self::IgetWide { field, .. }
            | Self::IgetObject { field, .. }
            | Self::IgetBoolean { field, .. }
            | Self::IgetByte { field, .. }
            | Self::IgetChar { field, .. }
            | Self::IgetShort { field, .. }
            | Self::Iput { field, .. }
            | Self::IputWide { field, .. }
            | Self::IputObject { field, .. }
            | Self::IputBoolean { field, .. }
            | Self::IputByte { field, .. }
            | Self::IputChar { field, .. }
            | Self::IputShort { field, .. }
            | Self::Sget { field, .. }
            | Self::SgetWide { field, .. }
            | Self::SgetObject { field, .. }
            | Self::SgetBoolean { field, .. }
            | Self::SgetByte { field, .. }
            | Self::SgetChar { field, .. }
            | Self::SgetShort { field, .. }
            | Self::Sput { field, .. }
            | Self::SputWide { field, .. }
            | Self::SputObject { field, .. }
            | Self::SputBoolean { field, .. }
            | Self::SputByte { field, .. }
            | Self::SputChar { field, .. }
            | Self::SputShort { field, .. } => Some(*field),
            _ => None,
        }
    }

    pub fn string_ref(&self) -> Option<StringIdx> {
        match self {
            Self::ConstString { string, .. } | Self::ConstStringJumbo { string, .. } => {
                Some(*string)
            }
            _ => None,
        }
    }

    pub fn type_ref(&self) -> Option<TypeIdx> {
        match self {
            Self::ConstClass { type_, .. }
            | Self::CheckCast { type_, .. }
            | Self::InstanceOf { type_, .. }
            | Self::NewInstance { type_, .. }
            | Self::NewArray { type_, .. }
            | Self::FilledNewArray { type_, .. }
            | Self::FilledNewArrayRange { type_, .. } => Some(*type_),
            _ => None,
        }
    }

    pub fn literal(&self) -> Option<i64> {
        match self {
            Self::Const4 { value, .. } => Some(i64::from(*value)),
            Self::Const16 { value, .. } => Some(i64::from(*value)),
            Self::Const { value, .. } => Some(i64::from(*value)),
            Self::ConstHigh16 { value, .. } => Some(i64::from(*value)),
            Self::ConstWide16 { value, .. } => Some(i64::from(*value)),
            Self::ConstWide32 { value, .. } => Some(i64::from(*value)),
            Self::ConstWide { value, .. } => Some(*value),
            Self::ConstWideHigh16 { value, .. } => Some(i64::from(*value)),
            Self::AddIntLit16 { literal, .. }
            | Self::RsubIntLit16 { literal, .. }
            | Self::MulIntLit16 { literal, .. }
            | Self::DivIntLit16 { literal, .. }
            | Self::RemIntLit16 { literal, .. }
            | Self::AndIntLit16 { literal, .. }
            | Self::OrIntLit16 { literal, .. }
            | Self::XorIntLit16 { literal, .. } => Some(i64::from(*literal)),
            Self::AddIntLit8 { literal, .. }
            | Self::RsubIntLit8 { literal, .. }
            | Self::MulIntLit8 { literal, .. }
            | Self::DivIntLit8 { literal, .. }
            | Self::RemIntLit8 { literal, .. }
            | Self::AndIntLit8 { literal, .. }
            | Self::OrIntLit8 { literal, .. }
            | Self::XorIntLit8 { literal, .. }
            | Self::ShlIntLit8 { literal, .. }
            | Self::ShrIntLit8 { literal, .. }
            | Self::UshrIntLit8 { literal, .. } => Some(i64::from(*literal)),
            _ => None,
        }
    }

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
            | Self::MoveException { dest } => {
                regs.push(u16::from(*dest));
            }
            Self::Return { src }
            | Self::ReturnWide { src }
            | Self::ReturnObject { src } => {
                regs.push(u16::from(*src));
            }
            Self::Const4 { dest, .. } => {
                regs.push(u16::from(*dest));
            }
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
            | Self::ConstMethodType { dest, .. } => {
                regs.push(u16::from(*dest));
            }
            Self::MonitorEnter { ref_ } | Self::MonitorExit { ref_ } => {
                regs.push(u16::from(*ref_));
            }
            Self::CheckCast { ref_, .. } => {
                regs.push(u16::from(*ref_));
            }
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
                for r in args {
                    regs.push(u16::from(*r));
                }
            }
            Self::FilledNewArrayRange { first_reg, count, .. } => {
                for i in 0..u16::from(*count) {
                    regs.push(*first_reg + i);
                }
            }
            Self::FillArrayData { array, .. } => {
                regs.push(u16::from(*array));
            }
            Self::Throw { exception } => {
                regs.push(u16::from(*exception));
            }
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
            | Self::IfLez { a, .. } => {
                regs.push(u16::from(*a));
            }
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
            | Self::SgetShort { dest, .. } => {
                regs.push(u16::from(*dest));
            }
            Self::Sput { src, .. }
            | Self::SputWide { src, .. }
            | Self::SputObject { src, .. }
            | Self::SputBoolean { src, .. }
            | Self::SputByte { src, .. }
            | Self::SputChar { src, .. }
            | Self::SputShort { src, .. } => {
                regs.push(u16::from(*src));
            }
            Self::InvokeVirtual { args, .. }
            | Self::InvokeSuper { args, .. }
            | Self::InvokeDirect { args, .. }
            | Self::InvokeStatic { args, .. }
            | Self::InvokeInterface { args, .. } => {
                for r in args {
                    regs.push(u16::from(*r));
                }
            }
            Self::InvokeVirtualRange { first_reg, count, .. }
            | Self::InvokeSuperRange { first_reg, count, .. }
            | Self::InvokeDirectRange { first_reg, count, .. }
            | Self::InvokeStaticRange { first_reg, count, .. }
            | Self::InvokeInterfaceRange { first_reg, count, .. } => {
                for i in 0..u16::from(*count) {
                    regs.push(*first_reg + i);
                }
            }
            Self::InvokePolymorphic { args, .. } => {
                for r in args {
                    regs.push(u16::from(*r));
                }
            }
            Self::InvokePolymorphicRange { first_reg, count, .. } => {
                for i in 0..u16::from(*count) {
                    regs.push(*first_reg + i);
                }
            }
            Self::InvokeCustom { args, .. } => {
                for r in args {
                    regs.push(u16::from(*r));
                }
            }
            Self::InvokeCustomRange { first_reg, count, .. } => {
                for i in 0..u16::from(*count) {
                    regs.push(*first_reg + i);
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

    pub fn is_invoke(&self) -> bool {
        matches!(
            self,
            Self::InvokeVirtual { .. }
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
        )
    }

    pub fn is_branch(&self) -> bool {
        matches!(
            self,
            Self::Goto { .. }
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
                | Self::PackedSwitch { .. }
                | Self::SparseSwitch { .. }
        )
    }

    pub fn is_return(&self) -> bool {
        matches!(
            self,
            Self::ReturnVoid | Self::Return { .. } | Self::ReturnWide { .. } | Self::ReturnObject { .. }
        )
    }
}
