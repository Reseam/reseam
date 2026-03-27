use boltffi::data;

#[data]
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub class_descriptor: String,
    pub method_name: String,
    pub proto: String,
    pub access_flags: u32,
    pub dex_index: u32,
    pub register_count: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub instruction_count: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub descriptor: String,
    pub access_flags: u32,
    pub superclass: Option<String>,
    pub interfaces: Vec<String>,
    pub dex_index: u32,
    pub direct_method_count: u32,
    pub virtual_method_count: u32,
    pub static_field_count: u32,
    pub instance_field_count: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub class_descriptor: String,
    pub name: String,
    pub field_type: String,
    pub access_flags: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct FingerprintDef {
    pub name: Option<String>,
    pub defining_class: Option<String>,
    pub access_flags: Option<u32>,
    pub return_type: Option<String>,
    pub parameters: Option<Vec<String>>,
    pub opcodes: Option<Vec<i32>>,
    pub strings: Option<Vec<String>>,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct FingerprintResult {
    pub method: u32,
    pub matched_count: u32,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct InstructionHit {
    pub method: u32,
    pub index: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct NewMethod {
    pub name: String,
    pub proto: String,
    pub access_flags: u32,
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub instructions: Vec<Instruction>,
    pub tries: Vec<TryItem>,
    pub catch_handlers: Vec<CatchHandler>,
}

#[data]
#[derive(Debug, Clone, Copy)]
pub struct TryItem {
    pub start_addr: u32,
    pub insn_count: u16,
    pub handler_idx: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct CatchHandler {
    pub typed_catches: Vec<TypedCatch>,
    pub catch_all_addr: Option<u32>,
}

#[data]
#[derive(Debug, Clone)]
pub struct TypedCatch {
    pub exception_type: String,
    pub addr: u32,
}

#[data]
#[derive(Debug, Clone)]
pub struct NewField {
    pub name: String,
    pub field_type: String,
    pub access_flags: u32,
    pub initial_value: Option<EncodedVal>,
}

#[data]
#[derive(Debug, Clone)]
pub enum EncodedVal {
    Null,
    BoolVal(bool),
    ByteVal(i8),
    ShortVal(i16),
    CharVal(u16),
    IntVal(i32),
    LongVal(i64),
    FloatVal(f32),
    DoubleVal(f64),
    StringVal(String),
    TypeVal(String),
}

#[data]
#[derive(Debug, Clone)]
pub struct AnnotationItem {
    pub visibility: u8,
    pub annotation_type: String,
    pub elements: Vec<AnnotationElement>,
}

#[data]
#[derive(Debug, Clone)]
pub struct AnnotationElement {
    pub name: String,
    pub value: EncodedVal,
}

#[data]
#[derive(Debug, Clone)]
pub struct ResourceRef {
    pub res_id: u32,
    pub package_id: u8,
    pub type_id: u8,
    pub entry_index: u16,
    pub key_name: String,
}

#[data]
#[derive(Debug, Clone)]
pub struct MethodRef {
    pub defining_class: String,
    pub name: String,
    pub proto: String,
}

#[data]
#[derive(Debug, Clone)]
pub struct FieldRef {
    pub defining_class: String,
    pub name: String,
    pub field_type: String,
}

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
