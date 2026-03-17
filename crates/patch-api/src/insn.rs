use crate::opcode;
use crate::types::{FieldRef, Instruction, MethodRef};

impl Instruction {
    pub fn nop() -> Self {
        Self::Simple(opcode::NOP)
    }

    pub fn return_void() -> Self {
        Self::Simple(opcode::RETURN_VOID)
    }

    pub fn return_reg(src: u16) -> Self {
        Self::Reg1((opcode::RETURN, src))
    }

    pub fn return_wide(src: u16) -> Self {
        Self::Reg1((opcode::RETURN_WIDE, src))
    }

    pub fn return_object(src: u16) -> Self {
        Self::Reg1((opcode::RETURN_OBJECT, src))
    }

    pub fn move_reg(dest: u16, src: u16) -> Self {
        Self::Reg2((opcode::MOVE, dest, src))
    }

    pub fn move_wide(dest: u16, src: u16) -> Self {
        Self::Reg2((opcode::MOVE_WIDE, dest, src))
    }

    pub fn move_object(dest: u16, src: u16) -> Self {
        Self::Reg2((opcode::MOVE_OBJECT, dest, src))
    }

    pub fn move_result(dest: u16) -> Self {
        Self::Reg1((opcode::MOVE_RESULT, dest))
    }

    pub fn move_result_wide(dest: u16) -> Self {
        Self::Reg1((opcode::MOVE_RESULT_WIDE, dest))
    }

    pub fn move_result_object(dest: u16) -> Self {
        Self::Reg1((opcode::MOVE_RESULT_OBJECT, dest))
    }

    pub fn move_exception(dest: u16) -> Self {
        Self::Reg1((opcode::MOVE_EXCEPTION, dest))
    }

    pub fn const_4(dest: u16, value: i64) -> Self {
        Self::RegLiteral((opcode::CONST_4, dest, 0, value))
    }

    pub fn const_16(dest: u16, value: i64) -> Self {
        Self::RegLiteral((opcode::CONST_16, dest, 0, value))
    }

    pub fn const_val(dest: u16, value: i64) -> Self {
        Self::RegLiteral((opcode::CONST, dest, 0, value))
    }

    pub fn const_high16(dest: u16, value: i64) -> Self {
        Self::RegLiteral((opcode::CONST_HIGH16, dest, 0, value))
    }

    pub fn const_wide_16(dest: u16, value: i64) -> Self {
        Self::RegLiteral((opcode::CONST_WIDE_16, dest, 0, value))
    }

    pub fn const_wide_32(dest: u16, value: i64) -> Self {
        Self::RegLiteral((opcode::CONST_WIDE_32, dest, 0, value))
    }

    pub fn const_wide(dest: u16, value: i64) -> Self {
        Self::RegLiteral((opcode::CONST_WIDE, dest, 0, value))
    }

    pub fn const_string(dest: u16, s: String) -> Self {
        Self::RegString((opcode::CONST_STRING, dest, s))
    }

    pub fn const_class(dest: u16, type_desc: String) -> Self {
        Self::RegType((opcode::CONST_CLASS, dest, 0, type_desc))
    }

    pub fn check_cast(reg: u16, type_desc: String) -> Self {
        Self::RegType((opcode::CHECK_CAST, reg, 0, type_desc))
    }

    pub fn instance_of(dest: u16, obj: u16, type_desc: String) -> Self {
        Self::RegType((opcode::INSTANCE_OF, dest, obj, type_desc))
    }

    pub fn new_instance(dest: u16, type_desc: String) -> Self {
        Self::RegType((opcode::NEW_INSTANCE, dest, 0, type_desc))
    }

    pub fn new_array(dest: u16, size: u16, type_desc: String) -> Self {
        Self::RegType((opcode::NEW_ARRAY, dest, size, type_desc))
    }

    pub fn array_length(dest: u16, array: u16) -> Self {
        Self::Reg2((opcode::ARRAY_LENGTH, dest, array))
    }

    pub fn throw(reg: u16) -> Self {
        Self::Reg1((opcode::THROW, reg))
    }

    pub fn monitor_enter(reg: u16) -> Self {
        Self::Reg1((opcode::MONITOR_ENTER, reg))
    }

    pub fn monitor_exit(reg: u16) -> Self {
        Self::Reg1((opcode::MONITOR_EXIT, reg))
    }

    pub fn goto(offset: i32) -> Self {
        Self::Branch0((opcode::GOTO, offset))
    }

    pub fn if_eq(a: u16, b: u16, offset: i32) -> Self {
        Self::Branch2((opcode::IF_EQ, a, b, offset))
    }

    pub fn if_ne(a: u16, b: u16, offset: i32) -> Self {
        Self::Branch2((opcode::IF_NE, a, b, offset))
    }

    pub fn if_lt(a: u16, b: u16, offset: i32) -> Self {
        Self::Branch2((opcode::IF_LT, a, b, offset))
    }

    pub fn if_ge(a: u16, b: u16, offset: i32) -> Self {
        Self::Branch2((opcode::IF_GE, a, b, offset))
    }

    pub fn if_gt(a: u16, b: u16, offset: i32) -> Self {
        Self::Branch2((opcode::IF_GT, a, b, offset))
    }

    pub fn if_le(a: u16, b: u16, offset: i32) -> Self {
        Self::Branch2((opcode::IF_LE, a, b, offset))
    }

    pub fn if_eqz(reg: u16, offset: i32) -> Self {
        Self::Branch((opcode::IF_EQZ, reg, offset))
    }

    pub fn if_nez(reg: u16, offset: i32) -> Self {
        Self::Branch((opcode::IF_NEZ, reg, offset))
    }

    pub fn if_ltz(reg: u16, offset: i32) -> Self {
        Self::Branch((opcode::IF_LTZ, reg, offset))
    }

    pub fn if_gez(reg: u16, offset: i32) -> Self {
        Self::Branch((opcode::IF_GEZ, reg, offset))
    }

    pub fn if_gtz(reg: u16, offset: i32) -> Self {
        Self::Branch((opcode::IF_GTZ, reg, offset))
    }

    pub fn if_lez(reg: u16, offset: i32) -> Self {
        Self::Branch((opcode::IF_LEZ, reg, offset))
    }

    pub fn iget(dest: u16, obj: u16, field: FieldRef) -> Self {
        Self::RegField((opcode::IGET, dest, obj, field))
    }

    pub fn iget_object(dest: u16, obj: u16, field: FieldRef) -> Self {
        Self::RegField((opcode::IGET_OBJECT, dest, obj, field))
    }

    pub fn iput(src: u16, obj: u16, field: FieldRef) -> Self {
        Self::RegField((opcode::IPUT, src, obj, field))
    }

    pub fn iput_object(src: u16, obj: u16, field: FieldRef) -> Self {
        Self::RegField((opcode::IPUT_OBJECT, src, obj, field))
    }

    pub fn sget(dest: u16, field: FieldRef) -> Self {
        Self::RegField((opcode::SGET, dest, 0, field))
    }

    pub fn sget_object(dest: u16, field: FieldRef) -> Self {
        Self::RegField((opcode::SGET_OBJECT, dest, 0, field))
    }

    pub fn sput(src: u16, field: FieldRef) -> Self {
        Self::RegField((opcode::SPUT, src, 0, field))
    }

    pub fn sput_object(src: u16, field: FieldRef) -> Self {
        Self::RegField((opcode::SPUT_OBJECT, src, 0, field))
    }

    pub fn invoke_virtual(args: Vec<u16>, method: MethodRef) -> Self {
        Self::Invoke((opcode::INVOKE_VIRTUAL, args, method))
    }

    pub fn invoke_super(args: Vec<u16>, method: MethodRef) -> Self {
        Self::Invoke((opcode::INVOKE_SUPER, args, method))
    }

    pub fn invoke_direct(args: Vec<u16>, method: MethodRef) -> Self {
        Self::Invoke((opcode::INVOKE_DIRECT, args, method))
    }

    pub fn invoke_static(args: Vec<u16>, method: MethodRef) -> Self {
        Self::Invoke((opcode::INVOKE_STATIC, args, method))
    }

    pub fn invoke_interface(args: Vec<u16>, method: MethodRef) -> Self {
        Self::Invoke((opcode::INVOKE_INTERFACE, args, method))
    }

    pub fn invoke_virtual_range(first_reg: u16, count: u16, method: MethodRef) -> Self {
        Self::InvokeRange((opcode::INVOKE_VIRTUAL_RANGE, first_reg, count, method))
    }

    pub fn invoke_super_range(first_reg: u16, count: u16, method: MethodRef) -> Self {
        Self::InvokeRange((opcode::INVOKE_SUPER_RANGE, first_reg, count, method))
    }

    pub fn invoke_direct_range(first_reg: u16, count: u16, method: MethodRef) -> Self {
        Self::InvokeRange((opcode::INVOKE_DIRECT_RANGE, first_reg, count, method))
    }

    pub fn invoke_static_range(first_reg: u16, count: u16, method: MethodRef) -> Self {
        Self::InvokeRange((opcode::INVOKE_STATIC_RANGE, first_reg, count, method))
    }

    pub fn invoke_interface_range(first_reg: u16, count: u16, method: MethodRef) -> Self {
        Self::InvokeRange((opcode::INVOKE_INTERFACE_RANGE, first_reg, count, method))
    }
}

impl MethodRef {
    pub fn new(class: &str, name: &str, proto: &str) -> Self {
        Self {
            defining_class: class.to_string(),
            name: name.to_string(),
            proto: proto.to_string(),
        }
    }
}

impl FieldRef {
    pub fn new(class: &str, name: &str, field_type: &str) -> Self {
        Self {
            defining_class: class.to_string(),
            name: name.to_string(),
            field_type: field_type.to_string(),
        }
    }
}
