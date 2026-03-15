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
}
