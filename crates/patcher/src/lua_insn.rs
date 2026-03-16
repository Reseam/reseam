use mlua::prelude::*;
use smallvec::SmallVec;
use stitch_apk::stitch_dex::{
    CallSiteIdx, FieldIdx, Instruction, InstructionPattern, MethodHandleIdx, MethodIdx,
    OpcodeMatcher, ProtoIdx, StringIdx, TypeIdx,
};

use Instruction as I;
use OpcodeMatcher as OM;

macro_rules! op {
    ($t:ident, $name:literal) => { $t.set("op", $name)? };
}
macro_rules! ds {
    ($t:ident, $d:expr, $s:expr) => { $t.set("dest", $d)?; $t.set("src", $s)? };
}
macro_rules! dab {
    ($t:ident, $d:expr, $a:expr, $b:expr) => { $t.set("dest", $d)?; $t.set("a", $a)?; $t.set("b", $b)? };
}
macro_rules! ab {
    ($t:ident, $a:expr, $b:expr) => { $t.set("dest_a", $a)?; $t.set("b", $b)? };
}
macro_rules! dsl {
    ($t:ident, $d:expr, $s:expr, $l:expr) => { $t.set("dest", $d)?; $t.set("src", $s)?; $t.set("literal", $l)? };
}

pub fn instruction_to_lua(lua: &Lua, insn: &I) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    match insn {
        I::Nop => op!(t, "nop"),
        I::ReturnVoid => op!(t, "return_void"),

        I::Move { dest, src } => { op!(t, "move"); ds!(t, *dest, *src); }
        I::MoveFrom16 { dest, src } => { op!(t, "move_from16"); ds!(t, *dest, *src); }
        I::Move16 { dest, src } => { op!(t, "move16"); ds!(t, *dest, *src); }
        I::MoveWide { dest, src } => { op!(t, "move_wide"); ds!(t, *dest, *src); }
        I::MoveWideFrom16 { dest, src } => { op!(t, "move_wide_from16"); ds!(t, *dest, *src); }
        I::MoveWide16 { dest, src } => { op!(t, "move_wide16"); ds!(t, *dest, *src); }
        I::MoveObject { dest, src } => { op!(t, "move_object"); ds!(t, *dest, *src); }
        I::MoveObjectFrom16 { dest, src } => { op!(t, "move_object_from16"); ds!(t, *dest, *src); }
        I::MoveObject16 { dest, src } => { op!(t, "move_object16"); ds!(t, *dest, *src); }

        I::MoveResult { dest } => { op!(t, "move_result"); t.set("dest", *dest)?; }
        I::MoveResultWide { dest } => { op!(t, "move_result_wide"); t.set("dest", *dest)?; }
        I::MoveResultObject { dest } => { op!(t, "move_result_object"); t.set("dest", *dest)?; }
        I::MoveException { dest } => { op!(t, "move_exception"); t.set("dest", *dest)?; }

        I::Return { src } => { op!(t, "return"); t.set("src", *src)?; }
        I::ReturnWide { src } => { op!(t, "return_wide"); t.set("src", *src)?; }
        I::ReturnObject { src } => { op!(t, "return_object"); t.set("src", *src)?; }

        I::Const4 { dest, value } => { op!(t, "const4"); t.set("dest", *dest)?; t.set("value", *value)?; }
        I::Const16 { dest, value } => { op!(t, "const16"); t.set("dest", *dest)?; t.set("value", *value)?; }
        I::Const { dest, value } => { op!(t, "const"); t.set("dest", *dest)?; t.set("value", *value)?; }
        I::ConstHigh16 { dest, value } => { op!(t, "const_high16"); t.set("dest", *dest)?; t.set("value", *value)?; }
        I::ConstWide16 { dest, value } => { op!(t, "const_wide16"); t.set("dest", *dest)?; t.set("value", *value)?; }
        I::ConstWide32 { dest, value } => { op!(t, "const_wide32"); t.set("dest", *dest)?; t.set("value", *value)?; }
        I::ConstWide { dest, value } => { op!(t, "const_wide"); t.set("dest", *dest)?; t.set("value", *value)?; }
        I::ConstWideHigh16 { dest, value } => { op!(t, "const_wide_high16"); t.set("dest", *dest)?; t.set("value", *value)?; }

        I::ConstString { dest, string } => { op!(t, "const_string"); t.set("dest", *dest)?; t.set("string_idx", string.0)?; }
        I::ConstStringJumbo { dest, string } => { op!(t, "const_string_jumbo"); t.set("dest", *dest)?; t.set("string_idx", string.0)?; }
        I::ConstClass { dest, type_ } => { op!(t, "const_class"); t.set("dest", *dest)?; t.set("type_idx", type_.0)?; }

        I::MonitorEnter { ref_ } => { op!(t, "monitor_enter"); t.set("ref_reg", *ref_)?; }
        I::MonitorExit { ref_ } => { op!(t, "monitor_exit"); t.set("ref_reg", *ref_)?; }

        I::CheckCast { ref_, type_ } => { op!(t, "check_cast"); t.set("ref_reg", *ref_)?; t.set("type_idx", type_.0)?; }
        I::InstanceOf { dest, ref_, type_ } => { op!(t, "instance_of"); t.set("dest", *dest)?; t.set("ref_reg", *ref_)?; t.set("type_idx", type_.0)?; }

        I::ArrayLength { dest, array } => { op!(t, "array_length"); t.set("dest", *dest)?; t.set("array", *array)?; }
        I::NewInstance { dest, type_ } => { op!(t, "new_instance"); t.set("dest", *dest)?; t.set("type_idx", type_.0)?; }
        I::NewArray { dest, size, type_ } => { op!(t, "new_array"); t.set("dest", *dest)?; t.set("size", *size)?; t.set("type_idx", type_.0)?; }
        I::FilledNewArray { type_, args } => { op!(t, "filled_new_array"); t.set("type_idx", type_.0)?; set_args(&t, args)?; }
        I::FilledNewArrayRange { type_, first_reg, count } => { op!(t, "filled_new_array_range"); t.set("type_idx", type_.0)?; t.set("first_reg", *first_reg)?; t.set("count", *count)?; }
        I::FillArrayData { array, payload_offset } => { op!(t, "fill_array_data"); t.set("array", *array)?; t.set("payload_offset", *payload_offset)?; }

        I::Throw { exception } => { op!(t, "throw"); t.set("exception", *exception)?; }

        I::Goto { offset } => { op!(t, "goto"); t.set("offset", *offset)?; }
        I::Goto16 { offset } => { op!(t, "goto16"); t.set("offset", *offset)?; }
        I::Goto32 { offset } => { op!(t, "goto32"); t.set("offset", *offset)?; }

        I::PackedSwitch { test, payload_offset } => { op!(t, "packed_switch"); t.set("test", *test)?; t.set("payload_offset", *payload_offset)?; }
        I::SparseSwitch { test, payload_offset } => { op!(t, "sparse_switch"); t.set("test", *test)?; t.set("payload_offset", *payload_offset)?; }

        I::CmpLFloat { dest, a, b } => { op!(t, "cmpl_float"); dab!(t, *dest, *a, *b); }
        I::CmpGFloat { dest, a, b } => { op!(t, "cmpg_float"); dab!(t, *dest, *a, *b); }
        I::CmpLDouble { dest, a, b } => { op!(t, "cmpl_double"); dab!(t, *dest, *a, *b); }
        I::CmpGDouble { dest, a, b } => { op!(t, "cmpg_double"); dab!(t, *dest, *a, *b); }
        I::CmpLong { dest, a, b } => { op!(t, "cmp_long"); dab!(t, *dest, *a, *b); }

        I::IfEq { a, b, offset } => { op!(t, "if_eq"); t.set("a", *a)?; t.set("b", *b)?; t.set("offset", *offset)?; }
        I::IfNe { a, b, offset } => { op!(t, "if_ne"); t.set("a", *a)?; t.set("b", *b)?; t.set("offset", *offset)?; }
        I::IfLt { a, b, offset } => { op!(t, "if_lt"); t.set("a", *a)?; t.set("b", *b)?; t.set("offset", *offset)?; }
        I::IfGe { a, b, offset } => { op!(t, "if_ge"); t.set("a", *a)?; t.set("b", *b)?; t.set("offset", *offset)?; }
        I::IfGt { a, b, offset } => { op!(t, "if_gt"); t.set("a", *a)?; t.set("b", *b)?; t.set("offset", *offset)?; }
        I::IfLe { a, b, offset } => { op!(t, "if_le"); t.set("a", *a)?; t.set("b", *b)?; t.set("offset", *offset)?; }

        I::IfEqz { a, offset } => { op!(t, "if_eqz"); t.set("a", *a)?; t.set("offset", *offset)?; }
        I::IfNez { a, offset } => { op!(t, "if_nez"); t.set("a", *a)?; t.set("offset", *offset)?; }
        I::IfLtz { a, offset } => { op!(t, "if_ltz"); t.set("a", *a)?; t.set("offset", *offset)?; }
        I::IfGez { a, offset } => { op!(t, "if_gez"); t.set("a", *a)?; t.set("offset", *offset)?; }
        I::IfGtz { a, offset } => { op!(t, "if_gtz"); t.set("a", *a)?; t.set("offset", *offset)?; }
        I::IfLez { a, offset } => { op!(t, "if_lez"); t.set("a", *a)?; t.set("offset", *offset)?; }

        I::Aget { dest, array, index } => { op!(t, "aget"); dab!(t, *dest, *array, *index); }
        I::AgetWide { dest, array, index } => { op!(t, "aget_wide"); dab!(t, *dest, *array, *index); }
        I::AgetObject { dest, array, index } => { op!(t, "aget_object"); dab!(t, *dest, *array, *index); }
        I::AgetBoolean { dest, array, index } => { op!(t, "aget_boolean"); dab!(t, *dest, *array, *index); }
        I::AgetByte { dest, array, index } => { op!(t, "aget_byte"); dab!(t, *dest, *array, *index); }
        I::AgetChar { dest, array, index } => { op!(t, "aget_char"); dab!(t, *dest, *array, *index); }
        I::AgetShort { dest, array, index } => { op!(t, "aget_short"); dab!(t, *dest, *array, *index); }
        I::Aput { src, array, index } => { op!(t, "aput"); dab!(t, *src, *array, *index); }
        I::AputWide { src, array, index } => { op!(t, "aput_wide"); dab!(t, *src, *array, *index); }
        I::AputObject { src, array, index } => { op!(t, "aput_object"); dab!(t, *src, *array, *index); }
        I::AputBoolean { src, array, index } => { op!(t, "aput_boolean"); dab!(t, *src, *array, *index); }
        I::AputByte { src, array, index } => { op!(t, "aput_byte"); dab!(t, *src, *array, *index); }
        I::AputChar { src, array, index } => { op!(t, "aput_char"); dab!(t, *src, *array, *index); }
        I::AputShort { src, array, index } => { op!(t, "aput_short"); dab!(t, *src, *array, *index); }

        I::Iget { dest, obj, field } => { op!(t, "iget"); t.set("dest", *dest)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IgetWide { dest, obj, field } => { op!(t, "iget_wide"); t.set("dest", *dest)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IgetObject { dest, obj, field } => { op!(t, "iget_object"); t.set("dest", *dest)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IgetBoolean { dest, obj, field } => { op!(t, "iget_boolean"); t.set("dest", *dest)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IgetByte { dest, obj, field } => { op!(t, "iget_byte"); t.set("dest", *dest)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IgetChar { dest, obj, field } => { op!(t, "iget_char"); t.set("dest", *dest)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IgetShort { dest, obj, field } => { op!(t, "iget_short"); t.set("dest", *dest)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::Iput { src, obj, field } => { op!(t, "iput"); t.set("src", *src)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IputWide { src, obj, field } => { op!(t, "iput_wide"); t.set("src", *src)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IputObject { src, obj, field } => { op!(t, "iput_object"); t.set("src", *src)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IputBoolean { src, obj, field } => { op!(t, "iput_boolean"); t.set("src", *src)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IputByte { src, obj, field } => { op!(t, "iput_byte"); t.set("src", *src)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IputChar { src, obj, field } => { op!(t, "iput_char"); t.set("src", *src)?; t.set("obj", *obj)?; t.set("field", field.0)?; }
        I::IputShort { src, obj, field } => { op!(t, "iput_short"); t.set("src", *src)?; t.set("obj", *obj)?; t.set("field", field.0)?; }

        I::Sget { dest, field } => { op!(t, "sget"); t.set("dest", *dest)?; t.set("field", field.0)?; }
        I::SgetWide { dest, field } => { op!(t, "sget_wide"); t.set("dest", *dest)?; t.set("field", field.0)?; }
        I::SgetObject { dest, field } => { op!(t, "sget_object"); t.set("dest", *dest)?; t.set("field", field.0)?; }
        I::SgetBoolean { dest, field } => { op!(t, "sget_boolean"); t.set("dest", *dest)?; t.set("field", field.0)?; }
        I::SgetByte { dest, field } => { op!(t, "sget_byte"); t.set("dest", *dest)?; t.set("field", field.0)?; }
        I::SgetChar { dest, field } => { op!(t, "sget_char"); t.set("dest", *dest)?; t.set("field", field.0)?; }
        I::SgetShort { dest, field } => { op!(t, "sget_short"); t.set("dest", *dest)?; t.set("field", field.0)?; }
        I::Sput { src, field } => { op!(t, "sput"); t.set("src", *src)?; t.set("field", field.0)?; }
        I::SputWide { src, field } => { op!(t, "sput_wide"); t.set("src", *src)?; t.set("field", field.0)?; }
        I::SputObject { src, field } => { op!(t, "sput_object"); t.set("src", *src)?; t.set("field", field.0)?; }
        I::SputBoolean { src, field } => { op!(t, "sput_boolean"); t.set("src", *src)?; t.set("field", field.0)?; }
        I::SputByte { src, field } => { op!(t, "sput_byte"); t.set("src", *src)?; t.set("field", field.0)?; }
        I::SputChar { src, field } => { op!(t, "sput_char"); t.set("src", *src)?; t.set("field", field.0)?; }
        I::SputShort { src, field } => { op!(t, "sput_short"); t.set("src", *src)?; t.set("field", field.0)?; }

        I::InvokeVirtual { method, args } => { op!(t, "invoke_virtual"); t.set("method", method.0)?; set_args(&t, args)?; }
        I::InvokeSuper { method, args } => { op!(t, "invoke_super"); t.set("method", method.0)?; set_args(&t, args)?; }
        I::InvokeDirect { method, args } => { op!(t, "invoke_direct"); t.set("method", method.0)?; set_args(&t, args)?; }
        I::InvokeStatic { method, args } => { op!(t, "invoke_static"); t.set("method", method.0)?; set_args(&t, args)?; }
        I::InvokeInterface { method, args } => { op!(t, "invoke_interface"); t.set("method", method.0)?; set_args(&t, args)?; }

        I::InvokeVirtualRange { method, first_reg, count } => { op!(t, "invoke_virtual_range"); t.set("method", method.0)?; t.set("first_reg", *first_reg)?; t.set("count", *count)?; }
        I::InvokeSuperRange { method, first_reg, count } => { op!(t, "invoke_super_range"); t.set("method", method.0)?; t.set("first_reg", *first_reg)?; t.set("count", *count)?; }
        I::InvokeDirectRange { method, first_reg, count } => { op!(t, "invoke_direct_range"); t.set("method", method.0)?; t.set("first_reg", *first_reg)?; t.set("count", *count)?; }
        I::InvokeStaticRange { method, first_reg, count } => { op!(t, "invoke_static_range"); t.set("method", method.0)?; t.set("first_reg", *first_reg)?; t.set("count", *count)?; }
        I::InvokeInterfaceRange { method, first_reg, count } => { op!(t, "invoke_interface_range"); t.set("method", method.0)?; t.set("first_reg", *first_reg)?; t.set("count", *count)?; }

        I::InvokePolymorphic { method, proto, args } => { op!(t, "invoke_polymorphic"); t.set("method", method.0)?; t.set("proto", proto.0)?; set_args(&t, args)?; }
        I::InvokePolymorphicRange { method, proto, first_reg, count } => { op!(t, "invoke_polymorphic_range"); t.set("method", method.0)?; t.set("proto", proto.0)?; t.set("first_reg", *first_reg)?; t.set("count", *count)?; }
        I::InvokeCustom { call_site, args } => { op!(t, "invoke_custom"); t.set("call_site", call_site.0)?; set_args(&t, args)?; }
        I::InvokeCustomRange { call_site, first_reg, count } => { op!(t, "invoke_custom_range"); t.set("call_site", call_site.0)?; t.set("first_reg", *first_reg)?; t.set("count", *count)?; }

        I::ConstMethodHandle { dest, method_handle } => { op!(t, "const_method_handle"); t.set("dest", *dest)?; t.set("method_handle", method_handle.0)?; }
        I::ConstMethodType { dest, proto } => { op!(t, "const_method_type"); t.set("dest", *dest)?; t.set("proto", proto.0)?; }

        I::NegInt { dest, src } => { op!(t, "neg_int"); ds!(t, *dest, *src); }
        I::NotInt { dest, src } => { op!(t, "not_int"); ds!(t, *dest, *src); }
        I::NegLong { dest, src } => { op!(t, "neg_long"); ds!(t, *dest, *src); }
        I::NotLong { dest, src } => { op!(t, "not_long"); ds!(t, *dest, *src); }
        I::NegFloat { dest, src } => { op!(t, "neg_float"); ds!(t, *dest, *src); }
        I::NegDouble { dest, src } => { op!(t, "neg_double"); ds!(t, *dest, *src); }
        I::IntToLong { dest, src } => { op!(t, "int_to_long"); ds!(t, *dest, *src); }
        I::IntToFloat { dest, src } => { op!(t, "int_to_float"); ds!(t, *dest, *src); }
        I::IntToDouble { dest, src } => { op!(t, "int_to_double"); ds!(t, *dest, *src); }
        I::LongToInt { dest, src } => { op!(t, "long_to_int"); ds!(t, *dest, *src); }
        I::LongToFloat { dest, src } => { op!(t, "long_to_float"); ds!(t, *dest, *src); }
        I::LongToDouble { dest, src } => { op!(t, "long_to_double"); ds!(t, *dest, *src); }
        I::FloatToInt { dest, src } => { op!(t, "float_to_int"); ds!(t, *dest, *src); }
        I::FloatToLong { dest, src } => { op!(t, "float_to_long"); ds!(t, *dest, *src); }
        I::FloatToDouble { dest, src } => { op!(t, "float_to_double"); ds!(t, *dest, *src); }
        I::DoubleToInt { dest, src } => { op!(t, "double_to_int"); ds!(t, *dest, *src); }
        I::DoubleToLong { dest, src } => { op!(t, "double_to_long"); ds!(t, *dest, *src); }
        I::DoubleToFloat { dest, src } => { op!(t, "double_to_float"); ds!(t, *dest, *src); }
        I::IntToByte { dest, src } => { op!(t, "int_to_byte"); ds!(t, *dest, *src); }
        I::IntToChar { dest, src } => { op!(t, "int_to_char"); ds!(t, *dest, *src); }
        I::IntToShort { dest, src } => { op!(t, "int_to_short"); ds!(t, *dest, *src); }

        I::AddInt { dest, a, b } => { op!(t, "add_int"); dab!(t, *dest, *a, *b); }
        I::SubInt { dest, a, b } => { op!(t, "sub_int"); dab!(t, *dest, *a, *b); }
        I::MulInt { dest, a, b } => { op!(t, "mul_int"); dab!(t, *dest, *a, *b); }
        I::DivInt { dest, a, b } => { op!(t, "div_int"); dab!(t, *dest, *a, *b); }
        I::RemInt { dest, a, b } => { op!(t, "rem_int"); dab!(t, *dest, *a, *b); }
        I::AndInt { dest, a, b } => { op!(t, "and_int"); dab!(t, *dest, *a, *b); }
        I::OrInt { dest, a, b } => { op!(t, "or_int"); dab!(t, *dest, *a, *b); }
        I::XorInt { dest, a, b } => { op!(t, "xor_int"); dab!(t, *dest, *a, *b); }
        I::ShlInt { dest, a, b } => { op!(t, "shl_int"); dab!(t, *dest, *a, *b); }
        I::ShrInt { dest, a, b } => { op!(t, "shr_int"); dab!(t, *dest, *a, *b); }
        I::UshrInt { dest, a, b } => { op!(t, "ushr_int"); dab!(t, *dest, *a, *b); }
        I::AddLong { dest, a, b } => { op!(t, "add_long"); dab!(t, *dest, *a, *b); }
        I::SubLong { dest, a, b } => { op!(t, "sub_long"); dab!(t, *dest, *a, *b); }
        I::MulLong { dest, a, b } => { op!(t, "mul_long"); dab!(t, *dest, *a, *b); }
        I::DivLong { dest, a, b } => { op!(t, "div_long"); dab!(t, *dest, *a, *b); }
        I::RemLong { dest, a, b } => { op!(t, "rem_long"); dab!(t, *dest, *a, *b); }
        I::AndLong { dest, a, b } => { op!(t, "and_long"); dab!(t, *dest, *a, *b); }
        I::OrLong { dest, a, b } => { op!(t, "or_long"); dab!(t, *dest, *a, *b); }
        I::XorLong { dest, a, b } => { op!(t, "xor_long"); dab!(t, *dest, *a, *b); }
        I::ShlLong { dest, a, b } => { op!(t, "shl_long"); dab!(t, *dest, *a, *b); }
        I::ShrLong { dest, a, b } => { op!(t, "shr_long"); dab!(t, *dest, *a, *b); }
        I::UshrLong { dest, a, b } => { op!(t, "ushr_long"); dab!(t, *dest, *a, *b); }
        I::AddFloat { dest, a, b } => { op!(t, "add_float"); dab!(t, *dest, *a, *b); }
        I::SubFloat { dest, a, b } => { op!(t, "sub_float"); dab!(t, *dest, *a, *b); }
        I::MulFloat { dest, a, b } => { op!(t, "mul_float"); dab!(t, *dest, *a, *b); }
        I::DivFloat { dest, a, b } => { op!(t, "div_float"); dab!(t, *dest, *a, *b); }
        I::RemFloat { dest, a, b } => { op!(t, "rem_float"); dab!(t, *dest, *a, *b); }
        I::AddDouble { dest, a, b } => { op!(t, "add_double"); dab!(t, *dest, *a, *b); }
        I::SubDouble { dest, a, b } => { op!(t, "sub_double"); dab!(t, *dest, *a, *b); }
        I::MulDouble { dest, a, b } => { op!(t, "mul_double"); dab!(t, *dest, *a, *b); }
        I::DivDouble { dest, a, b } => { op!(t, "div_double"); dab!(t, *dest, *a, *b); }
        I::RemDouble { dest, a, b } => { op!(t, "rem_double"); dab!(t, *dest, *a, *b); }

        I::AddInt2Addr { dest_a, b } => { op!(t, "add_int_2addr"); ab!(t, *dest_a, *b); }
        I::SubInt2Addr { dest_a, b } => { op!(t, "sub_int_2addr"); ab!(t, *dest_a, *b); }
        I::MulInt2Addr { dest_a, b } => { op!(t, "mul_int_2addr"); ab!(t, *dest_a, *b); }
        I::DivInt2Addr { dest_a, b } => { op!(t, "div_int_2addr"); ab!(t, *dest_a, *b); }
        I::RemInt2Addr { dest_a, b } => { op!(t, "rem_int_2addr"); ab!(t, *dest_a, *b); }
        I::AndInt2Addr { dest_a, b } => { op!(t, "and_int_2addr"); ab!(t, *dest_a, *b); }
        I::OrInt2Addr { dest_a, b } => { op!(t, "or_int_2addr"); ab!(t, *dest_a, *b); }
        I::XorInt2Addr { dest_a, b } => { op!(t, "xor_int_2addr"); ab!(t, *dest_a, *b); }
        I::ShlInt2Addr { dest_a, b } => { op!(t, "shl_int_2addr"); ab!(t, *dest_a, *b); }
        I::ShrInt2Addr { dest_a, b } => { op!(t, "shr_int_2addr"); ab!(t, *dest_a, *b); }
        I::UshrInt2Addr { dest_a, b } => { op!(t, "ushr_int_2addr"); ab!(t, *dest_a, *b); }
        I::AddLong2Addr { dest_a, b } => { op!(t, "add_long_2addr"); ab!(t, *dest_a, *b); }
        I::SubLong2Addr { dest_a, b } => { op!(t, "sub_long_2addr"); ab!(t, *dest_a, *b); }
        I::MulLong2Addr { dest_a, b } => { op!(t, "mul_long_2addr"); ab!(t, *dest_a, *b); }
        I::DivLong2Addr { dest_a, b } => { op!(t, "div_long_2addr"); ab!(t, *dest_a, *b); }
        I::RemLong2Addr { dest_a, b } => { op!(t, "rem_long_2addr"); ab!(t, *dest_a, *b); }
        I::AndLong2Addr { dest_a, b } => { op!(t, "and_long_2addr"); ab!(t, *dest_a, *b); }
        I::OrLong2Addr { dest_a, b } => { op!(t, "or_long_2addr"); ab!(t, *dest_a, *b); }
        I::XorLong2Addr { dest_a, b } => { op!(t, "xor_long_2addr"); ab!(t, *dest_a, *b); }
        I::ShlLong2Addr { dest_a, b } => { op!(t, "shl_long_2addr"); ab!(t, *dest_a, *b); }
        I::ShrLong2Addr { dest_a, b } => { op!(t, "shr_long_2addr"); ab!(t, *dest_a, *b); }
        I::UshrLong2Addr { dest_a, b } => { op!(t, "ushr_long_2addr"); ab!(t, *dest_a, *b); }
        I::AddFloat2Addr { dest_a, b } => { op!(t, "add_float_2addr"); ab!(t, *dest_a, *b); }
        I::SubFloat2Addr { dest_a, b } => { op!(t, "sub_float_2addr"); ab!(t, *dest_a, *b); }
        I::MulFloat2Addr { dest_a, b } => { op!(t, "mul_float_2addr"); ab!(t, *dest_a, *b); }
        I::DivFloat2Addr { dest_a, b } => { op!(t, "div_float_2addr"); ab!(t, *dest_a, *b); }
        I::RemFloat2Addr { dest_a, b } => { op!(t, "rem_float_2addr"); ab!(t, *dest_a, *b); }
        I::AddDouble2Addr { dest_a, b } => { op!(t, "add_double_2addr"); ab!(t, *dest_a, *b); }
        I::SubDouble2Addr { dest_a, b } => { op!(t, "sub_double_2addr"); ab!(t, *dest_a, *b); }
        I::MulDouble2Addr { dest_a, b } => { op!(t, "mul_double_2addr"); ab!(t, *dest_a, *b); }
        I::DivDouble2Addr { dest_a, b } => { op!(t, "div_double_2addr"); ab!(t, *dest_a, *b); }
        I::RemDouble2Addr { dest_a, b } => { op!(t, "rem_double_2addr"); ab!(t, *dest_a, *b); }

        I::AddIntLit16 { dest, src, literal } => { op!(t, "add_int_lit16"); dsl!(t, *dest, *src, *literal); }
        I::RsubIntLit16 { dest, src, literal } => { op!(t, "rsub_int_lit16"); dsl!(t, *dest, *src, *literal); }
        I::MulIntLit16 { dest, src, literal } => { op!(t, "mul_int_lit16"); dsl!(t, *dest, *src, *literal); }
        I::DivIntLit16 { dest, src, literal } => { op!(t, "div_int_lit16"); dsl!(t, *dest, *src, *literal); }
        I::RemIntLit16 { dest, src, literal } => { op!(t, "rem_int_lit16"); dsl!(t, *dest, *src, *literal); }
        I::AndIntLit16 { dest, src, literal } => { op!(t, "and_int_lit16"); dsl!(t, *dest, *src, *literal); }
        I::OrIntLit16 { dest, src, literal } => { op!(t, "or_int_lit16"); dsl!(t, *dest, *src, *literal); }
        I::XorIntLit16 { dest, src, literal } => { op!(t, "xor_int_lit16"); dsl!(t, *dest, *src, *literal); }
        I::AddIntLit8 { dest, src, literal } => { op!(t, "add_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::RsubIntLit8 { dest, src, literal } => { op!(t, "rsub_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::MulIntLit8 { dest, src, literal } => { op!(t, "mul_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::DivIntLit8 { dest, src, literal } => { op!(t, "div_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::RemIntLit8 { dest, src, literal } => { op!(t, "rem_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::AndIntLit8 { dest, src, literal } => { op!(t, "and_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::OrIntLit8 { dest, src, literal } => { op!(t, "or_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::XorIntLit8 { dest, src, literal } => { op!(t, "xor_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::ShlIntLit8 { dest, src, literal } => { op!(t, "shl_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::ShrIntLit8 { dest, src, literal } => { op!(t, "shr_int_lit8"); dsl!(t, *dest, *src, *literal); }
        I::UshrIntLit8 { dest, src, literal } => { op!(t, "ushr_int_lit8"); dsl!(t, *dest, *src, *literal); }

        I::PackedSwitchPayload { first_key, targets } => {
            op!(t, "packed_switch_payload");
            t.set("first_key", *first_key)?;
            t.set("targets", targets.clone())?;
        }
        I::SparseSwitchPayload { keys_and_targets } => {
            op!(t, "sparse_switch_payload");
            let entries: Vec<(i32, i32)> = keys_and_targets.clone();
            let tbl = lua.create_table()?;
            for (i, (key, target)) in entries.iter().enumerate() {
                let e = lua.create_table()?;
                e.set("key", *key)?;
                e.set("target", *target)?;
                tbl.set(i + 1, e)?;
            }
            t.set("entries", tbl)?;
        }
        I::FillArrayDataPayload { element_width, data } => {
            op!(t, "fill_array_data_payload");
            t.set("element_width", *element_width)?;
            t.set("data", data.clone())?;
        }
        I::RawInstruction { code_units } => {
            op!(t, "raw");
            t.set("code_units", code_units.to_vec())?;
        }
        _ => return Err(LuaError::runtime("unsupported instruction variant")),
    }
    Ok(t)
}

pub fn lua_to_instruction(tbl: &LuaTable) -> LuaResult<I> {
    let op: String = tbl.get("op")?;
    match op.as_str() {
        "nop" => Ok(I::Nop),
        "return_void" => Ok(I::ReturnVoid),

        "move" => Ok(I::Move { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "move_from16" => Ok(I::MoveFrom16 { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "move16" => Ok(I::Move16 { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "move_wide" => Ok(I::MoveWide { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "move_wide_from16" => Ok(I::MoveWideFrom16 { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "move_wide16" => Ok(I::MoveWide16 { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "move_object" => Ok(I::MoveObject { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "move_object_from16" => Ok(I::MoveObjectFrom16 { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "move_object16" => Ok(I::MoveObject16 { dest: tbl.get("dest")?, src: tbl.get("src")? }),

        "move_result" => Ok(I::MoveResult { dest: tbl.get("dest")? }),
        "move_result_wide" => Ok(I::MoveResultWide { dest: tbl.get("dest")? }),
        "move_result_object" => Ok(I::MoveResultObject { dest: tbl.get("dest")? }),
        "move_exception" => Ok(I::MoveException { dest: tbl.get("dest")? }),

        "return" => Ok(I::Return { src: tbl.get("src")? }),
        "return_wide" => Ok(I::ReturnWide { src: tbl.get("src")? }),
        "return_object" => Ok(I::ReturnObject { src: tbl.get("src")? }),

        "const4" => Ok(I::Const4 { dest: tbl.get("dest")?, value: tbl.get("value")? }),
        "const16" => Ok(I::Const16 { dest: tbl.get("dest")?, value: tbl.get("value")? }),
        "const" => Ok(I::Const { dest: tbl.get("dest")?, value: tbl.get("value")? }),
        "const_high16" => Ok(I::ConstHigh16 { dest: tbl.get("dest")?, value: tbl.get("value")? }),
        "const_wide16" => Ok(I::ConstWide16 { dest: tbl.get("dest")?, value: tbl.get("value")? }),
        "const_wide32" => Ok(I::ConstWide32 { dest: tbl.get("dest")?, value: tbl.get("value")? }),
        "const_wide" => Ok(I::ConstWide { dest: tbl.get("dest")?, value: tbl.get("value")? }),
        "const_wide_high16" => Ok(I::ConstWideHigh16 { dest: tbl.get("dest")?, value: tbl.get("value")? }),

        "const_string" => Ok(I::ConstString { dest: tbl.get("dest")?, string: StringIdx(tbl.get("string_idx")?) }),
        "const_string_jumbo" => Ok(I::ConstStringJumbo { dest: tbl.get("dest")?, string: StringIdx(tbl.get("string_idx")?) }),
        "const_class" => Ok(I::ConstClass { dest: tbl.get("dest")?, type_: TypeIdx(tbl.get("type_idx")?) }),

        "monitor_enter" => Ok(I::MonitorEnter { ref_: tbl.get("ref_reg")? }),
        "monitor_exit" => Ok(I::MonitorExit { ref_: tbl.get("ref_reg")? }),

        "check_cast" => Ok(I::CheckCast { ref_: tbl.get("ref_reg")?, type_: TypeIdx(tbl.get("type_idx")?) }),
        "instance_of" => Ok(I::InstanceOf { dest: tbl.get("dest")?, ref_: tbl.get("ref_reg")?, type_: TypeIdx(tbl.get("type_idx")?) }),

        "array_length" => Ok(I::ArrayLength { dest: tbl.get("dest")?, array: tbl.get("array")? }),
        "new_instance" => Ok(I::NewInstance { dest: tbl.get("dest")?, type_: TypeIdx(tbl.get("type_idx")?) }),
        "new_array" => Ok(I::NewArray { dest: tbl.get("dest")?, size: tbl.get("size")?, type_: TypeIdx(tbl.get("type_idx")?) }),
        "filled_new_array" => Ok(I::FilledNewArray { type_: TypeIdx(tbl.get("type_idx")?), args: get_args(tbl)? }),
        "filled_new_array_range" => Ok(I::FilledNewArrayRange { type_: TypeIdx(tbl.get("type_idx")?), first_reg: tbl.get("first_reg")?, count: tbl.get("count")? }),
        "fill_array_data" => Ok(I::FillArrayData { array: tbl.get("array")?, payload_offset: tbl.get("payload_offset")? }),

        "throw" => Ok(I::Throw { exception: tbl.get("exception")? }),

        "goto" => Ok(I::Goto { offset: tbl.get("offset")? }),
        "goto16" => Ok(I::Goto16 { offset: tbl.get("offset")? }),
        "goto32" => Ok(I::Goto32 { offset: tbl.get("offset")? }),

        "packed_switch" => Ok(I::PackedSwitch { test: tbl.get("test")?, payload_offset: tbl.get("payload_offset")? }),
        "sparse_switch" => Ok(I::SparseSwitch { test: tbl.get("test")?, payload_offset: tbl.get("payload_offset")? }),

        "cmpl_float" => Ok(I::CmpLFloat { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "cmpg_float" => Ok(I::CmpGFloat { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "cmpl_double" => Ok(I::CmpLDouble { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "cmpg_double" => Ok(I::CmpGDouble { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "cmp_long" => Ok(I::CmpLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),

        "if_eq" => Ok(I::IfEq { a: tbl.get("a")?, b: tbl.get("b")?, offset: tbl.get("offset")? }),
        "if_ne" => Ok(I::IfNe { a: tbl.get("a")?, b: tbl.get("b")?, offset: tbl.get("offset")? }),
        "if_lt" => Ok(I::IfLt { a: tbl.get("a")?, b: tbl.get("b")?, offset: tbl.get("offset")? }),
        "if_ge" => Ok(I::IfGe { a: tbl.get("a")?, b: tbl.get("b")?, offset: tbl.get("offset")? }),
        "if_gt" => Ok(I::IfGt { a: tbl.get("a")?, b: tbl.get("b")?, offset: tbl.get("offset")? }),
        "if_le" => Ok(I::IfLe { a: tbl.get("a")?, b: tbl.get("b")?, offset: tbl.get("offset")? }),

        "if_eqz" => Ok(I::IfEqz { a: tbl.get("a")?, offset: tbl.get("offset")? }),
        "if_nez" => Ok(I::IfNez { a: tbl.get("a")?, offset: tbl.get("offset")? }),
        "if_ltz" => Ok(I::IfLtz { a: tbl.get("a")?, offset: tbl.get("offset")? }),
        "if_gez" => Ok(I::IfGez { a: tbl.get("a")?, offset: tbl.get("offset")? }),
        "if_gtz" => Ok(I::IfGtz { a: tbl.get("a")?, offset: tbl.get("offset")? }),
        "if_lez" => Ok(I::IfLez { a: tbl.get("a")?, offset: tbl.get("offset")? }),

        "aget" => Ok(I::Aget { dest: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aget_wide" => Ok(I::AgetWide { dest: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aget_object" => Ok(I::AgetObject { dest: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aget_boolean" => Ok(I::AgetBoolean { dest: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aget_byte" => Ok(I::AgetByte { dest: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aget_char" => Ok(I::AgetChar { dest: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aget_short" => Ok(I::AgetShort { dest: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aput" => Ok(I::Aput { src: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aput_wide" => Ok(I::AputWide { src: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aput_object" => Ok(I::AputObject { src: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aput_boolean" => Ok(I::AputBoolean { src: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aput_byte" => Ok(I::AputByte { src: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aput_char" => Ok(I::AputChar { src: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),
        "aput_short" => Ok(I::AputShort { src: tbl.get("dest")?, array: tbl.get("a")?, index: tbl.get("b")? }),

        "iget" => Ok(I::Iget { dest: tbl.get("dest")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iget_wide" => Ok(I::IgetWide { dest: tbl.get("dest")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iget_object" => Ok(I::IgetObject { dest: tbl.get("dest")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iget_boolean" => Ok(I::IgetBoolean { dest: tbl.get("dest")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iget_byte" => Ok(I::IgetByte { dest: tbl.get("dest")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iget_char" => Ok(I::IgetChar { dest: tbl.get("dest")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iget_short" => Ok(I::IgetShort { dest: tbl.get("dest")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iput" => Ok(I::Iput { src: tbl.get("src")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iput_wide" => Ok(I::IputWide { src: tbl.get("src")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iput_object" => Ok(I::IputObject { src: tbl.get("src")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iput_boolean" => Ok(I::IputBoolean { src: tbl.get("src")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iput_byte" => Ok(I::IputByte { src: tbl.get("src")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iput_char" => Ok(I::IputChar { src: tbl.get("src")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),
        "iput_short" => Ok(I::IputShort { src: tbl.get("src")?, obj: tbl.get("obj")?, field: FieldIdx(tbl.get("field")?) }),

        "sget" => Ok(I::Sget { dest: tbl.get("dest")?, field: FieldIdx(tbl.get("field")?) }),
        "sget_wide" => Ok(I::SgetWide { dest: tbl.get("dest")?, field: FieldIdx(tbl.get("field")?) }),
        "sget_object" => Ok(I::SgetObject { dest: tbl.get("dest")?, field: FieldIdx(tbl.get("field")?) }),
        "sget_boolean" => Ok(I::SgetBoolean { dest: tbl.get("dest")?, field: FieldIdx(tbl.get("field")?) }),
        "sget_byte" => Ok(I::SgetByte { dest: tbl.get("dest")?, field: FieldIdx(tbl.get("field")?) }),
        "sget_char" => Ok(I::SgetChar { dest: tbl.get("dest")?, field: FieldIdx(tbl.get("field")?) }),
        "sget_short" => Ok(I::SgetShort { dest: tbl.get("dest")?, field: FieldIdx(tbl.get("field")?) }),
        "sput" => Ok(I::Sput { src: tbl.get("src")?, field: FieldIdx(tbl.get("field")?) }),
        "sput_wide" => Ok(I::SputWide { src: tbl.get("src")?, field: FieldIdx(tbl.get("field")?) }),
        "sput_object" => Ok(I::SputObject { src: tbl.get("src")?, field: FieldIdx(tbl.get("field")?) }),
        "sput_boolean" => Ok(I::SputBoolean { src: tbl.get("src")?, field: FieldIdx(tbl.get("field")?) }),
        "sput_byte" => Ok(I::SputByte { src: tbl.get("src")?, field: FieldIdx(tbl.get("field")?) }),
        "sput_char" => Ok(I::SputChar { src: tbl.get("src")?, field: FieldIdx(tbl.get("field")?) }),
        "sput_short" => Ok(I::SputShort { src: tbl.get("src")?, field: FieldIdx(tbl.get("field")?) }),

        "invoke_virtual" => Ok(I::InvokeVirtual { method: MethodIdx(tbl.get("method")?), args: get_args(tbl)? }),
        "invoke_super" => Ok(I::InvokeSuper { method: MethodIdx(tbl.get("method")?), args: get_args(tbl)? }),
        "invoke_direct" => Ok(I::InvokeDirect { method: MethodIdx(tbl.get("method")?), args: get_args(tbl)? }),
        "invoke_static" => Ok(I::InvokeStatic { method: MethodIdx(tbl.get("method")?), args: get_args(tbl)? }),
        "invoke_interface" => Ok(I::InvokeInterface { method: MethodIdx(tbl.get("method")?), args: get_args(tbl)? }),

        "invoke_virtual_range" => Ok(I::InvokeVirtualRange { method: MethodIdx(tbl.get("method")?), first_reg: tbl.get("first_reg")?, count: tbl.get("count")? }),
        "invoke_super_range" => Ok(I::InvokeSuperRange { method: MethodIdx(tbl.get("method")?), first_reg: tbl.get("first_reg")?, count: tbl.get("count")? }),
        "invoke_direct_range" => Ok(I::InvokeDirectRange { method: MethodIdx(tbl.get("method")?), first_reg: tbl.get("first_reg")?, count: tbl.get("count")? }),
        "invoke_static_range" => Ok(I::InvokeStaticRange { method: MethodIdx(tbl.get("method")?), first_reg: tbl.get("first_reg")?, count: tbl.get("count")? }),
        "invoke_interface_range" => Ok(I::InvokeInterfaceRange { method: MethodIdx(tbl.get("method")?), first_reg: tbl.get("first_reg")?, count: tbl.get("count")? }),

        "invoke_polymorphic" => Ok(I::InvokePolymorphic { method: MethodIdx(tbl.get("method")?), proto: ProtoIdx(tbl.get("proto")?), args: get_args(tbl)? }),
        "invoke_polymorphic_range" => Ok(I::InvokePolymorphicRange { method: MethodIdx(tbl.get("method")?), proto: ProtoIdx(tbl.get("proto")?), first_reg: tbl.get("first_reg")?, count: tbl.get("count")? }),
        "invoke_custom" => Ok(I::InvokeCustom { call_site: CallSiteIdx(tbl.get("call_site")?), args: get_args(tbl)? }),
        "invoke_custom_range" => Ok(I::InvokeCustomRange { call_site: CallSiteIdx(tbl.get("call_site")?), first_reg: tbl.get("first_reg")?, count: tbl.get("count")? }),

        "const_method_handle" => Ok(I::ConstMethodHandle { dest: tbl.get("dest")?, method_handle: MethodHandleIdx(tbl.get("method_handle")?) }),
        "const_method_type" => Ok(I::ConstMethodType { dest: tbl.get("dest")?, proto: ProtoIdx(tbl.get("proto")?) }),

        "neg_int" => Ok(I::NegInt { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "not_int" => Ok(I::NotInt { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "neg_long" => Ok(I::NegLong { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "not_long" => Ok(I::NotLong { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "neg_float" => Ok(I::NegFloat { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "neg_double" => Ok(I::NegDouble { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "int_to_long" => Ok(I::IntToLong { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "int_to_float" => Ok(I::IntToFloat { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "int_to_double" => Ok(I::IntToDouble { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "long_to_int" => Ok(I::LongToInt { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "long_to_float" => Ok(I::LongToFloat { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "long_to_double" => Ok(I::LongToDouble { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "float_to_int" => Ok(I::FloatToInt { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "float_to_long" => Ok(I::FloatToLong { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "float_to_double" => Ok(I::FloatToDouble { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "double_to_int" => Ok(I::DoubleToInt { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "double_to_long" => Ok(I::DoubleToLong { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "double_to_float" => Ok(I::DoubleToFloat { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "int_to_byte" => Ok(I::IntToByte { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "int_to_char" => Ok(I::IntToChar { dest: tbl.get("dest")?, src: tbl.get("src")? }),
        "int_to_short" => Ok(I::IntToShort { dest: tbl.get("dest")?, src: tbl.get("src")? }),

        "add_int" => Ok(I::AddInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "sub_int" => Ok(I::SubInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "mul_int" => Ok(I::MulInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "div_int" => Ok(I::DivInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "rem_int" => Ok(I::RemInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "and_int" => Ok(I::AndInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "or_int" => Ok(I::OrInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "xor_int" => Ok(I::XorInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "shl_int" => Ok(I::ShlInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "shr_int" => Ok(I::ShrInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "ushr_int" => Ok(I::UshrInt { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "add_long" => Ok(I::AddLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "sub_long" => Ok(I::SubLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "mul_long" => Ok(I::MulLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "div_long" => Ok(I::DivLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "rem_long" => Ok(I::RemLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "and_long" => Ok(I::AndLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "or_long" => Ok(I::OrLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "xor_long" => Ok(I::XorLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "shl_long" => Ok(I::ShlLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "shr_long" => Ok(I::ShrLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "ushr_long" => Ok(I::UshrLong { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "add_float" => Ok(I::AddFloat { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "sub_float" => Ok(I::SubFloat { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "mul_float" => Ok(I::MulFloat { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "div_float" => Ok(I::DivFloat { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "rem_float" => Ok(I::RemFloat { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "add_double" => Ok(I::AddDouble { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "sub_double" => Ok(I::SubDouble { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "mul_double" => Ok(I::MulDouble { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "div_double" => Ok(I::DivDouble { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),
        "rem_double" => Ok(I::RemDouble { dest: tbl.get("dest")?, a: tbl.get("a")?, b: tbl.get("b")? }),

        "add_int_2addr" => Ok(I::AddInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "sub_int_2addr" => Ok(I::SubInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "mul_int_2addr" => Ok(I::MulInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "div_int_2addr" => Ok(I::DivInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "rem_int_2addr" => Ok(I::RemInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "and_int_2addr" => Ok(I::AndInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "or_int_2addr" => Ok(I::OrInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "xor_int_2addr" => Ok(I::XorInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "shl_int_2addr" => Ok(I::ShlInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "shr_int_2addr" => Ok(I::ShrInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "ushr_int_2addr" => Ok(I::UshrInt2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "add_long_2addr" => Ok(I::AddLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "sub_long_2addr" => Ok(I::SubLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "mul_long_2addr" => Ok(I::MulLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "div_long_2addr" => Ok(I::DivLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "rem_long_2addr" => Ok(I::RemLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "and_long_2addr" => Ok(I::AndLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "or_long_2addr" => Ok(I::OrLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "xor_long_2addr" => Ok(I::XorLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "shl_long_2addr" => Ok(I::ShlLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "shr_long_2addr" => Ok(I::ShrLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "ushr_long_2addr" => Ok(I::UshrLong2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "add_float_2addr" => Ok(I::AddFloat2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "sub_float_2addr" => Ok(I::SubFloat2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "mul_float_2addr" => Ok(I::MulFloat2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "div_float_2addr" => Ok(I::DivFloat2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "rem_float_2addr" => Ok(I::RemFloat2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "add_double_2addr" => Ok(I::AddDouble2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "sub_double_2addr" => Ok(I::SubDouble2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "mul_double_2addr" => Ok(I::MulDouble2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "div_double_2addr" => Ok(I::DivDouble2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),
        "rem_double_2addr" => Ok(I::RemDouble2Addr { dest_a: tbl.get("dest_a")?, b: tbl.get("b")? }),

        "add_int_lit16" => Ok(I::AddIntLit16 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "rsub_int_lit16" => Ok(I::RsubIntLit16 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "mul_int_lit16" => Ok(I::MulIntLit16 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "div_int_lit16" => Ok(I::DivIntLit16 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "rem_int_lit16" => Ok(I::RemIntLit16 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "and_int_lit16" => Ok(I::AndIntLit16 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "or_int_lit16" => Ok(I::OrIntLit16 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "xor_int_lit16" => Ok(I::XorIntLit16 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "add_int_lit8" => Ok(I::AddIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "rsub_int_lit8" => Ok(I::RsubIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "mul_int_lit8" => Ok(I::MulIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "div_int_lit8" => Ok(I::DivIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "rem_int_lit8" => Ok(I::RemIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "and_int_lit8" => Ok(I::AndIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "or_int_lit8" => Ok(I::OrIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "xor_int_lit8" => Ok(I::XorIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "shl_int_lit8" => Ok(I::ShlIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "shr_int_lit8" => Ok(I::ShrIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),
        "ushr_int_lit8" => Ok(I::UshrIntLit8 { dest: tbl.get("dest")?, src: tbl.get("src")?, literal: tbl.get("literal")? }),

        "packed_switch_payload" => Ok(I::PackedSwitchPayload { first_key: tbl.get("first_key")?, targets: tbl.get("targets")? }),
        "sparse_switch_payload" => {
            let entries: LuaTable = tbl.get("entries")?;
            let mut kv = Vec::new();
            for pair in entries.sequence_values::<LuaTable>() {
                let e = pair?;
                kv.push((e.get("key")?, e.get("target")?));
            }
            Ok(I::SparseSwitchPayload { keys_and_targets: kv })
        }
        "fill_array_data_payload" => Ok(I::FillArrayDataPayload { element_width: tbl.get("element_width")?, data: tbl.get("data")? }),
        "raw" => {
            let units: Vec<u16> = tbl.get("code_units")?;
            Ok(I::RawInstruction { code_units: SmallVec::from_vec(units) })
        }

        other => Err(LuaError::runtime(format!("unknown opcode: {other}")))
    }
}

pub fn parse_pattern(name: &str) -> LuaResult<InstructionPattern> {
    if name == "any" {
        return Ok(InstructionPattern::Any);
    }
    let m = match name {
        "nop" => OM::Nop, "move" => OM::Move, "move_from16" => OM::MoveFrom16, "move16" => OM::Move16,
        "move_wide" => OM::MoveWide, "move_wide_from16" => OM::MoveWideFrom16, "move_wide16" => OM::MoveWide16,
        "move_object" => OM::MoveObject, "move_object_from16" => OM::MoveObjectFrom16, "move_object16" => OM::MoveObject16,
        "move_result" => OM::MoveResult, "move_result_wide" => OM::MoveResultWide, "move_result_object" => OM::MoveResultObject,
        "move_exception" => OM::MoveException, "return_void" => OM::ReturnVoid, "return" => OM::Return,
        "return_wide" => OM::ReturnWide, "return_object" => OM::ReturnObject,
        "const4" => OM::Const4, "const16" => OM::Const16, "const" => OM::Const, "const_high16" => OM::ConstHigh16,
        "const_wide16" => OM::ConstWide16, "const_wide32" => OM::ConstWide32, "const_wide" => OM::ConstWide, "const_wide_high16" => OM::ConstWideHigh16,
        "const_string" => OM::ConstString, "const_string_jumbo" => OM::ConstStringJumbo, "const_class" => OM::ConstClass,
        "monitor_enter" => OM::MonitorEnter, "monitor_exit" => OM::MonitorExit,
        "check_cast" => OM::CheckCast, "instance_of" => OM::InstanceOf, "array_length" => OM::ArrayLength,
        "new_instance" => OM::NewInstance, "new_array" => OM::NewArray,
        "filled_new_array" => OM::FilledNewArray, "filled_new_array_range" => OM::FilledNewArrayRange,
        "fill_array_data" => OM::FillArrayData, "throw" => OM::Throw,
        "goto" => OM::Goto, "goto16" => OM::Goto16, "goto32" => OM::Goto32,
        "packed_switch" => OM::PackedSwitch, "sparse_switch" => OM::SparseSwitch,
        "cmpl_float" => OM::CmpLFloat, "cmpg_float" => OM::CmpGFloat, "cmpl_double" => OM::CmpLDouble,
        "cmpg_double" => OM::CmpGDouble, "cmp_long" => OM::CmpLong,
        "if_eq" => OM::IfEq, "if_ne" => OM::IfNe, "if_lt" => OM::IfLt,
        "if_ge" => OM::IfGe, "if_gt" => OM::IfGt, "if_le" => OM::IfLe,
        "if_eqz" => OM::IfEqz, "if_nez" => OM::IfNez, "if_ltz" => OM::IfLtz,
        "if_gez" => OM::IfGez, "if_gtz" => OM::IfGtz, "if_lez" => OM::IfLez,
        "aget" => OM::Aget, "aget_wide" => OM::AgetWide, "aget_object" => OM::AgetObject,
        "aget_boolean" => OM::AgetBoolean, "aget_byte" => OM::AgetByte, "aget_char" => OM::AgetChar, "aget_short" => OM::AgetShort,
        "aput" => OM::Aput, "aput_wide" => OM::AputWide, "aput_object" => OM::AputObject,
        "aput_boolean" => OM::AputBoolean, "aput_byte" => OM::AputByte, "aput_char" => OM::AputChar, "aput_short" => OM::AputShort,
        "iget" => OM::Iget, "iget_wide" => OM::IgetWide, "iget_object" => OM::IgetObject,
        "iget_boolean" => OM::IgetBoolean, "iget_byte" => OM::IgetByte, "iget_char" => OM::IgetChar, "iget_short" => OM::IgetShort,
        "iput" => OM::Iput, "iput_wide" => OM::IputWide, "iput_object" => OM::IputObject,
        "iput_boolean" => OM::IputBoolean, "iput_byte" => OM::IputByte, "iput_char" => OM::IputChar, "iput_short" => OM::IputShort,
        "sget" => OM::Sget, "sget_wide" => OM::SgetWide, "sget_object" => OM::SgetObject,
        "sget_boolean" => OM::SgetBoolean, "sget_byte" => OM::SgetByte, "sget_char" => OM::SgetChar, "sget_short" => OM::SgetShort,
        "sput" => OM::Sput, "sput_wide" => OM::SputWide, "sput_object" => OM::SputObject,
        "sput_boolean" => OM::SputBoolean, "sput_byte" => OM::SputByte, "sput_char" => OM::SputChar, "sput_short" => OM::SputShort,
        "invoke_virtual" => OM::InvokeVirtual, "invoke_super" => OM::InvokeSuper, "invoke_direct" => OM::InvokeDirect,
        "invoke_static" => OM::InvokeStatic, "invoke_interface" => OM::InvokeInterface,
        "invoke_virtual_range" => OM::InvokeVirtualRange, "invoke_super_range" => OM::InvokeSuperRange,
        "invoke_direct_range" => OM::InvokeDirectRange, "invoke_static_range" => OM::InvokeStaticRange,
        "invoke_interface_range" => OM::InvokeInterfaceRange,
        "invoke_polymorphic" => OM::InvokePolymorphic, "invoke_polymorphic_range" => OM::InvokePolymorphicRange,
        "invoke_custom" => OM::InvokeCustom, "invoke_custom_range" => OM::InvokeCustomRange,
        "const_method_handle" => OM::ConstMethodHandle, "const_method_type" => OM::ConstMethodType,
        "neg_int" => OM::NegInt, "not_int" => OM::NotInt, "neg_long" => OM::NegLong, "not_long" => OM::NotLong,
        "neg_float" => OM::NegFloat, "neg_double" => OM::NegDouble,
        "int_to_long" => OM::IntToLong, "int_to_float" => OM::IntToFloat, "int_to_double" => OM::IntToDouble,
        "long_to_int" => OM::LongToInt, "long_to_float" => OM::LongToFloat, "long_to_double" => OM::LongToDouble,
        "float_to_int" => OM::FloatToInt, "float_to_long" => OM::FloatToLong, "float_to_double" => OM::FloatToDouble,
        "double_to_int" => OM::DoubleToInt, "double_to_long" => OM::DoubleToLong, "double_to_float" => OM::DoubleToFloat,
        "int_to_byte" => OM::IntToByte, "int_to_char" => OM::IntToChar, "int_to_short" => OM::IntToShort,
        "add_int" => OM::AddInt, "sub_int" => OM::SubInt, "mul_int" => OM::MulInt, "div_int" => OM::DivInt, "rem_int" => OM::RemInt,
        "and_int" => OM::AndInt, "or_int" => OM::OrInt, "xor_int" => OM::XorInt,
        "shl_int" => OM::ShlInt, "shr_int" => OM::ShrInt, "ushr_int" => OM::UshrInt,
        "add_long" => OM::AddLong, "sub_long" => OM::SubLong, "mul_long" => OM::MulLong, "div_long" => OM::DivLong, "rem_long" => OM::RemLong,
        "and_long" => OM::AndLong, "or_long" => OM::OrLong, "xor_long" => OM::XorLong,
        "shl_long" => OM::ShlLong, "shr_long" => OM::ShrLong, "ushr_long" => OM::UshrLong,
        "add_float" => OM::AddFloat, "sub_float" => OM::SubFloat, "mul_float" => OM::MulFloat, "div_float" => OM::DivFloat, "rem_float" => OM::RemFloat,
        "add_double" => OM::AddDouble, "sub_double" => OM::SubDouble, "mul_double" => OM::MulDouble, "div_double" => OM::DivDouble, "rem_double" => OM::RemDouble,
        "add_int_2addr" => OM::AddInt2Addr, "sub_int_2addr" => OM::SubInt2Addr, "mul_int_2addr" => OM::MulInt2Addr,
        "div_int_2addr" => OM::DivInt2Addr, "rem_int_2addr" => OM::RemInt2Addr, "and_int_2addr" => OM::AndInt2Addr,
        "or_int_2addr" => OM::OrInt2Addr, "xor_int_2addr" => OM::XorInt2Addr, "shl_int_2addr" => OM::ShlInt2Addr,
        "shr_int_2addr" => OM::ShrInt2Addr, "ushr_int_2addr" => OM::UshrInt2Addr,
        "add_long_2addr" => OM::AddLong2Addr, "sub_long_2addr" => OM::SubLong2Addr, "mul_long_2addr" => OM::MulLong2Addr,
        "div_long_2addr" => OM::DivLong2Addr, "rem_long_2addr" => OM::RemLong2Addr, "and_long_2addr" => OM::AndLong2Addr,
        "or_long_2addr" => OM::OrLong2Addr, "xor_long_2addr" => OM::XorLong2Addr, "shl_long_2addr" => OM::ShlLong2Addr,
        "shr_long_2addr" => OM::ShrLong2Addr, "ushr_long_2addr" => OM::UshrLong2Addr,
        "add_float_2addr" => OM::AddFloat2Addr, "sub_float_2addr" => OM::SubFloat2Addr, "mul_float_2addr" => OM::MulFloat2Addr,
        "div_float_2addr" => OM::DivFloat2Addr, "rem_float_2addr" => OM::RemFloat2Addr,
        "add_double_2addr" => OM::AddDouble2Addr, "sub_double_2addr" => OM::SubDouble2Addr, "mul_double_2addr" => OM::MulDouble2Addr,
        "div_double_2addr" => OM::DivDouble2Addr, "rem_double_2addr" => OM::RemDouble2Addr,
        "add_int_lit16" => OM::AddIntLit16, "rsub_int_lit16" => OM::RsubIntLit16, "mul_int_lit16" => OM::MulIntLit16,
        "div_int_lit16" => OM::DivIntLit16, "rem_int_lit16" => OM::RemIntLit16, "and_int_lit16" => OM::AndIntLit16,
        "or_int_lit16" => OM::OrIntLit16, "xor_int_lit16" => OM::XorIntLit16,
        "add_int_lit8" => OM::AddIntLit8, "rsub_int_lit8" => OM::RsubIntLit8, "mul_int_lit8" => OM::MulIntLit8,
        "div_int_lit8" => OM::DivIntLit8, "rem_int_lit8" => OM::RemIntLit8, "and_int_lit8" => OM::AndIntLit8,
        "or_int_lit8" => OM::OrIntLit8, "xor_int_lit8" => OM::XorIntLit8,
        "shl_int_lit8" => OM::ShlIntLit8, "shr_int_lit8" => OM::ShrIntLit8, "ushr_int_lit8" => OM::UshrIntLit8,
        "packed_switch_payload" => OM::PackedSwitchPayload, "sparse_switch_payload" => OM::SparseSwitchPayload,
        "fill_array_data_payload" => OM::FillArrayDataPayload, "raw" => OM::RawInstruction,
        other => return Err(LuaError::runtime(format!("unknown opcode pattern: {other}")))
    };
    Ok(InstructionPattern::Opcode(m))
}

fn set_args(t: &LuaTable, args: &SmallVec<[u8; 5]>) -> LuaResult<()> {
    t.set("args", args.to_vec())
}

fn get_args(tbl: &LuaTable) -> LuaResult<SmallVec<[u8; 5]>> {
    let v: Vec<u8> = tbl.get("args")?;
    Ok(SmallVec::from_vec(v))
}
