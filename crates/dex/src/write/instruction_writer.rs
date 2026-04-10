use crate::error::Result;
use crate::types::instruction::Instruction;

pub fn encode_instructions(instructions: &[Instruction]) -> Result<Vec<u16>> {
    let mut code: Vec<u16> = Vec::new();
    for insn in instructions {
        encode_instruction(&mut code, insn)?;
    }
    Ok(code)
}

fn encode_instruction(code: &mut Vec<u16>, insn: &Instruction) -> Result<()> {
    match insn {
        Instruction::Nop => code.push(0x0000),

        // 12x: B|A|op
        Instruction::Move { dest, src } => code.push(pack_12x(0x01, *dest, *src)),
        Instruction::MoveWide { dest, src } => code.push(pack_12x(0x04, *dest, *src)),
        Instruction::MoveObject { dest, src } => code.push(pack_12x(0x07, *dest, *src)),
        Instruction::ArrayLength { dest, array } => code.push(pack_12x(0x21, *dest, *array)),

        // 22x: AA|op BBBB
        Instruction::MoveFrom16 { dest, src } => {
            code.push(pack_aa_op(0x02, *dest));
            code.push(*src);
        }
        Instruction::MoveWideFrom16 { dest, src } => {
            code.push(pack_aa_op(0x05, *dest));
            code.push(*src);
        }
        Instruction::MoveObjectFrom16 { dest, src } => {
            code.push(pack_aa_op(0x08, *dest));
            code.push(*src);
        }

        // 32x: 00|op AAAA BBBB
        Instruction::Move16 { dest, src } => {
            code.push(0x03);
            code.push(*dest);
            code.push(*src);
        }
        Instruction::MoveWide16 { dest, src } => {
            code.push(0x06);
            code.push(*dest);
            code.push(*src);
        }
        Instruction::MoveObject16 { dest, src } => {
            code.push(0x09);
            code.push(*dest);
            code.push(*src);
        }

        // 11x: AA|op
        Instruction::MoveResult { dest } => code.push(pack_aa_op(0x0a, *dest)),
        Instruction::MoveResultWide { dest } => code.push(pack_aa_op(0x0b, *dest)),
        Instruction::MoveResultObject { dest } => code.push(pack_aa_op(0x0c, *dest)),
        Instruction::MoveException { dest } => code.push(pack_aa_op(0x0d, *dest)),

        Instruction::ReturnVoid => code.push(0x0e),
        Instruction::Return { src } => code.push(pack_aa_op(0x0f, *src)),
        Instruction::ReturnWide { src } => code.push(pack_aa_op(0x10, *src)),
        Instruction::ReturnObject { src } => code.push(pack_aa_op(0x11, *src)),

        // 11n: B|A|op
        Instruction::Const4 { dest, value } => {
            let v = (*value as u8) & 0xF;
            code.push(0x12 | ((*dest as u16) << 8) | ((v as u16) << 12));
        }

        // 21s
        Instruction::Const16 { dest, value } => {
            code.push(pack_aa_op(0x13, *dest));
            code.push(*value as u16);
        }
        Instruction::ConstWide16 { dest, value } => {
            code.push(pack_aa_op(0x16, *dest));
            code.push(*value as u16);
        }

        // 31i
        Instruction::Const { dest, value } => {
            code.push(pack_aa_op(0x14, *dest));
            code.push(*value as u16);
            code.push((*value >> 16) as u16);
        }
        Instruction::ConstWide32 { dest, value } => {
            code.push(pack_aa_op(0x17, *dest));
            code.push(*value as u16);
            code.push((*value >> 16) as u16);
        }

        // 21h
        Instruction::ConstHigh16 { dest, value } => {
            code.push(pack_aa_op(0x15, *dest));
            code.push(*value as u16);
        }
        Instruction::ConstWideHigh16 { dest, value } => {
            code.push(pack_aa_op(0x19, *dest));
            code.push(*value as u16);
        }

        // 51l
        Instruction::ConstWide { dest, value } => {
            code.push(pack_aa_op(0x18, *dest));
            code.push(*value as u16);
            code.push((*value >> 16) as u16);
            code.push((*value >> 32) as u16);
            code.push((*value >> 48) as u16);
        }

        // 21c
        Instruction::ConstString { dest, string } => {
            code.push(pack_aa_op(0x1a, *dest));
            code.push(string.0 as u16);
        }
        Instruction::ConstClass { dest, type_ } => {
            code.push(pack_aa_op(0x1c, *dest));
            code.push(type_.0 as u16);
        }
        Instruction::ConstMethodHandle {
            dest,
            method_handle,
        } => {
            code.push(pack_aa_op(0xfe, *dest));
            code.push(method_handle.0 as u16);
        }
        Instruction::ConstMethodType { dest, proto } => {
            code.push(pack_aa_op(0xff, *dest));
            code.push(proto.0);
        }

        // 31c
        Instruction::ConstStringJumbo { dest, string } => {
            code.push(pack_aa_op(0x1b, *dest));
            code.push(string.0 as u16);
            code.push((string.0 >> 16) as u16);
        }

        // 11x
        Instruction::MonitorEnter { ref_ } => code.push(pack_aa_op(0x1d, *ref_)),
        Instruction::MonitorExit { ref_ } => code.push(pack_aa_op(0x1e, *ref_)),

        // 21c
        Instruction::CheckCast { ref_, type_ } => {
            code.push(pack_aa_op(0x1f, *ref_));
            code.push(type_.0 as u16);
        }

        // 22c
        Instruction::InstanceOf { dest, ref_, type_ } => {
            code.push(pack_12x(0x20, *dest, *ref_));
            code.push(type_.0 as u16);
        }
        Instruction::NewInstance { dest, type_ } => {
            code.push(pack_aa_op(0x22, *dest));
            code.push(type_.0 as u16);
        }
        Instruction::NewArray { dest, size, type_ } => {
            code.push(pack_12x(0x23, *dest, *size));
            code.push(type_.0 as u16);
        }

        // 35c
        Instruction::FilledNewArray { type_, args } => {
            encode_35c(code, 0x24, type_.0 as u16, args)?;
        }

        // 3rc
        Instruction::FilledNewArrayRange {
            type_,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x25, *count));
            code.push(type_.0 as u16);
            code.push(*first_reg);
        }

        // 31t
        Instruction::FillArrayData {
            array,
            payload_offset,
        } => {
            code.push(pack_aa_op(0x26, *array));
            code.push(*payload_offset as u16);
            code.push((*payload_offset >> 16) as u16);
        }

        Instruction::Throw { exception } => code.push(pack_aa_op(0x27, *exception)),

        // 10t
        Instruction::Goto { offset } => code.push(pack_aa_op(0x28, *offset as u8)),

        // 20t
        Instruction::Goto16 { offset } => {
            code.push(0x29);
            code.push(*offset as u16);
        }

        // 30t
        Instruction::Goto32 { offset } => {
            code.push(0x2a);
            code.push(*offset as u16);
            code.push((*offset >> 16) as u16);
        }

        // 31t
        Instruction::PackedSwitch {
            test,
            payload_offset,
        } => {
            code.push(pack_aa_op(0x2b, *test));
            code.push(*payload_offset as u16);
            code.push((*payload_offset >> 16) as u16);
        }
        Instruction::SparseSwitch {
            test,
            payload_offset,
        } => {
            code.push(pack_aa_op(0x2c, *test));
            code.push(*payload_offset as u16);
            code.push((*payload_offset >> 16) as u16);
        }

        // 23x: cmp
        Instruction::CmpLFloat { dest, a, b } => encode_23x(code, 0x2d, *dest, *a, *b),
        Instruction::CmpGFloat { dest, a, b } => encode_23x(code, 0x2e, *dest, *a, *b),
        Instruction::CmpLDouble { dest, a, b } => encode_23x(code, 0x2f, *dest, *a, *b),
        Instruction::CmpGDouble { dest, a, b } => encode_23x(code, 0x30, *dest, *a, *b),
        Instruction::CmpLong { dest, a, b } => encode_23x(code, 0x31, *dest, *a, *b),

        // 22t: if-test
        Instruction::IfEq { a, b, offset } => {
            code.push(pack_12x(0x32, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfNe { a, b, offset } => {
            code.push(pack_12x(0x33, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfLt { a, b, offset } => {
            code.push(pack_12x(0x34, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfGe { a, b, offset } => {
            code.push(pack_12x(0x35, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfGt { a, b, offset } => {
            code.push(pack_12x(0x36, *a, *b));
            code.push(*offset as u16);
        }
        Instruction::IfLe { a, b, offset } => {
            code.push(pack_12x(0x37, *a, *b));
            code.push(*offset as u16);
        }

        // 21t: if-testz
        Instruction::IfEqz { a, offset } => {
            code.push(pack_aa_op(0x38, *a));
            code.push(*offset as u16);
        }
        Instruction::IfNez { a, offset } => {
            code.push(pack_aa_op(0x39, *a));
            code.push(*offset as u16);
        }
        Instruction::IfLtz { a, offset } => {
            code.push(pack_aa_op(0x3a, *a));
            code.push(*offset as u16);
        }
        Instruction::IfGez { a, offset } => {
            code.push(pack_aa_op(0x3b, *a));
            code.push(*offset as u16);
        }
        Instruction::IfGtz { a, offset } => {
            code.push(pack_aa_op(0x3c, *a));
            code.push(*offset as u16);
        }
        Instruction::IfLez { a, offset } => {
            code.push(pack_aa_op(0x3d, *a));
            code.push(*offset as u16);
        }

        // 23x: array ops
        Instruction::Aget { dest, array, index } => encode_23x(code, 0x44, *dest, *array, *index),
        Instruction::AgetWide { dest, array, index } => {
            encode_23x(code, 0x45, *dest, *array, *index)
        }
        Instruction::AgetObject { dest, array, index } => {
            encode_23x(code, 0x46, *dest, *array, *index)
        }
        Instruction::AgetBoolean { dest, array, index } => {
            encode_23x(code, 0x47, *dest, *array, *index)
        }
        Instruction::AgetByte { dest, array, index } => {
            encode_23x(code, 0x48, *dest, *array, *index)
        }
        Instruction::AgetChar { dest, array, index } => {
            encode_23x(code, 0x49, *dest, *array, *index)
        }
        Instruction::AgetShort { dest, array, index } => {
            encode_23x(code, 0x4a, *dest, *array, *index)
        }
        Instruction::Aput { src, array, index } => encode_23x(code, 0x4b, *src, *array, *index),
        Instruction::AputWide { src, array, index } => encode_23x(code, 0x4c, *src, *array, *index),
        Instruction::AputObject { src, array, index } => {
            encode_23x(code, 0x4d, *src, *array, *index)
        }
        Instruction::AputBoolean { src, array, index } => {
            encode_23x(code, 0x4e, *src, *array, *index)
        }
        Instruction::AputByte { src, array, index } => encode_23x(code, 0x4f, *src, *array, *index),
        Instruction::AputChar { src, array, index } => encode_23x(code, 0x50, *src, *array, *index),
        Instruction::AputShort { src, array, index } => {
            encode_23x(code, 0x51, *src, *array, *index)
        }

        // 22c: instance field ops
        Instruction::Iget { dest, obj, field } => {
            code.push(pack_12x(0x52, *dest, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IgetWide { dest, obj, field } => {
            code.push(pack_12x(0x53, *dest, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IgetObject { dest, obj, field } => {
            code.push(pack_12x(0x54, *dest, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IgetBoolean { dest, obj, field } => {
            code.push(pack_12x(0x55, *dest, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IgetByte { dest, obj, field } => {
            code.push(pack_12x(0x56, *dest, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IgetChar { dest, obj, field } => {
            code.push(pack_12x(0x57, *dest, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IgetShort { dest, obj, field } => {
            code.push(pack_12x(0x58, *dest, *obj));
            code.push(field.0 as u16);
        }
        Instruction::Iput { src, obj, field } => {
            code.push(pack_12x(0x59, *src, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IputWide { src, obj, field } => {
            code.push(pack_12x(0x5a, *src, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IputObject { src, obj, field } => {
            code.push(pack_12x(0x5b, *src, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IputBoolean { src, obj, field } => {
            code.push(pack_12x(0x5c, *src, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IputByte { src, obj, field } => {
            code.push(pack_12x(0x5d, *src, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IputChar { src, obj, field } => {
            code.push(pack_12x(0x5e, *src, *obj));
            code.push(field.0 as u16);
        }
        Instruction::IputShort { src, obj, field } => {
            code.push(pack_12x(0x5f, *src, *obj));
            code.push(field.0 as u16);
        }

        // 21c: static field ops
        Instruction::Sget { dest, field } => {
            code.push(pack_aa_op(0x60, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetWide { dest, field } => {
            code.push(pack_aa_op(0x61, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetObject { dest, field } => {
            code.push(pack_aa_op(0x62, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetBoolean { dest, field } => {
            code.push(pack_aa_op(0x63, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetByte { dest, field } => {
            code.push(pack_aa_op(0x64, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetChar { dest, field } => {
            code.push(pack_aa_op(0x65, *dest));
            code.push(field.0 as u16);
        }
        Instruction::SgetShort { dest, field } => {
            code.push(pack_aa_op(0x66, *dest));
            code.push(field.0 as u16);
        }
        Instruction::Sput { src, field } => {
            code.push(pack_aa_op(0x67, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputWide { src, field } => {
            code.push(pack_aa_op(0x68, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputObject { src, field } => {
            code.push(pack_aa_op(0x69, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputBoolean { src, field } => {
            code.push(pack_aa_op(0x6a, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputByte { src, field } => {
            code.push(pack_aa_op(0x6b, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputChar { src, field } => {
            code.push(pack_aa_op(0x6c, *src));
            code.push(field.0 as u16);
        }
        Instruction::SputShort { src, field } => {
            code.push(pack_aa_op(0x6d, *src));
            code.push(field.0 as u16);
        }

        // 35c: invoke-kind
        Instruction::InvokeVirtual { method, args } => {
            encode_35c(code, 0x6e, method.0 as u16, args)?
        }
        Instruction::InvokeSuper { method, args } => encode_35c(code, 0x6f, method.0 as u16, args)?,
        Instruction::InvokeDirect { method, args } => {
            encode_35c(code, 0x70, method.0 as u16, args)?
        }
        Instruction::InvokeStatic { method, args } => {
            encode_35c(code, 0x71, method.0 as u16, args)?
        }
        Instruction::InvokeInterface { method, args } => {
            encode_35c(code, 0x72, method.0 as u16, args)?
        }

        // 3rc: invoke-kind/range
        Instruction::InvokeVirtualRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x74, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokeSuperRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x75, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokeDirectRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x76, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokeStaticRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x77, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }
        Instruction::InvokeInterfaceRange {
            method,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0x78, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
        }

        // 45cc: invoke-polymorphic
        Instruction::InvokePolymorphic {
            method,
            proto,
            args,
        } => {
            validate_35c_args(args)?;
            let count = args.len() as u8;
            let (c, d, e, f, g) = unpack_args(args);
            code.push(0xfa | ((count as u16) << 12) | ((g as u16) << 8));
            code.push(method.0 as u16);
            code.push((c as u16) | ((d as u16) << 4) | ((e as u16) << 8) | ((f as u16) << 12));
            code.push(proto.0);
        }
        Instruction::InvokePolymorphicRange {
            method,
            proto,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0xfb, *count));
            code.push(method.0 as u16);
            code.push(*first_reg);
            code.push(proto.0);
        }

        // 35c: invoke-custom
        Instruction::InvokeCustom { call_site, args } => {
            encode_35c(code, 0xfc, call_site.0 as u16, args)?
        }
        Instruction::InvokeCustomRange {
            call_site,
            first_reg,
            count,
        } => {
            code.push(pack_aa_op(0xfd, *count));
            code.push(call_site.0 as u16);
            code.push(*first_reg);
        }

        // 12x: unary ops
        Instruction::NegInt { dest, src } => code.push(pack_12x(0x7b, *dest, *src)),
        Instruction::NotInt { dest, src } => code.push(pack_12x(0x7c, *dest, *src)),
        Instruction::NegLong { dest, src } => code.push(pack_12x(0x7d, *dest, *src)),
        Instruction::NotLong { dest, src } => code.push(pack_12x(0x7e, *dest, *src)),
        Instruction::NegFloat { dest, src } => code.push(pack_12x(0x7f, *dest, *src)),
        Instruction::NegDouble { dest, src } => code.push(pack_12x(0x80, *dest, *src)),
        Instruction::IntToLong { dest, src } => code.push(pack_12x(0x81, *dest, *src)),
        Instruction::IntToFloat { dest, src } => code.push(pack_12x(0x82, *dest, *src)),
        Instruction::IntToDouble { dest, src } => code.push(pack_12x(0x83, *dest, *src)),
        Instruction::LongToInt { dest, src } => code.push(pack_12x(0x84, *dest, *src)),
        Instruction::LongToFloat { dest, src } => code.push(pack_12x(0x85, *dest, *src)),
        Instruction::LongToDouble { dest, src } => code.push(pack_12x(0x86, *dest, *src)),
        Instruction::FloatToInt { dest, src } => code.push(pack_12x(0x87, *dest, *src)),
        Instruction::FloatToLong { dest, src } => code.push(pack_12x(0x88, *dest, *src)),
        Instruction::FloatToDouble { dest, src } => code.push(pack_12x(0x89, *dest, *src)),
        Instruction::DoubleToInt { dest, src } => code.push(pack_12x(0x8a, *dest, *src)),
        Instruction::DoubleToLong { dest, src } => code.push(pack_12x(0x8b, *dest, *src)),
        Instruction::DoubleToFloat { dest, src } => code.push(pack_12x(0x8c, *dest, *src)),
        Instruction::IntToByte { dest, src } => code.push(pack_12x(0x8d, *dest, *src)),
        Instruction::IntToChar { dest, src } => code.push(pack_12x(0x8e, *dest, *src)),
        Instruction::IntToShort { dest, src } => code.push(pack_12x(0x8f, *dest, *src)),

        // 23x: binary ops
        Instruction::AddInt { dest, a, b } => encode_23x(code, 0x90, *dest, *a, *b),
        Instruction::SubInt { dest, a, b } => encode_23x(code, 0x91, *dest, *a, *b),
        Instruction::MulInt { dest, a, b } => encode_23x(code, 0x92, *dest, *a, *b),
        Instruction::DivInt { dest, a, b } => encode_23x(code, 0x93, *dest, *a, *b),
        Instruction::RemInt { dest, a, b } => encode_23x(code, 0x94, *dest, *a, *b),
        Instruction::AndInt { dest, a, b } => encode_23x(code, 0x95, *dest, *a, *b),
        Instruction::OrInt { dest, a, b } => encode_23x(code, 0x96, *dest, *a, *b),
        Instruction::XorInt { dest, a, b } => encode_23x(code, 0x97, *dest, *a, *b),
        Instruction::ShlInt { dest, a, b } => encode_23x(code, 0x98, *dest, *a, *b),
        Instruction::ShrInt { dest, a, b } => encode_23x(code, 0x99, *dest, *a, *b),
        Instruction::UshrInt { dest, a, b } => encode_23x(code, 0x9a, *dest, *a, *b),
        Instruction::AddLong { dest, a, b } => encode_23x(code, 0x9b, *dest, *a, *b),
        Instruction::SubLong { dest, a, b } => encode_23x(code, 0x9c, *dest, *a, *b),
        Instruction::MulLong { dest, a, b } => encode_23x(code, 0x9d, *dest, *a, *b),
        Instruction::DivLong { dest, a, b } => encode_23x(code, 0x9e, *dest, *a, *b),
        Instruction::RemLong { dest, a, b } => encode_23x(code, 0x9f, *dest, *a, *b),
        Instruction::AndLong { dest, a, b } => encode_23x(code, 0xa0, *dest, *a, *b),
        Instruction::OrLong { dest, a, b } => encode_23x(code, 0xa1, *dest, *a, *b),
        Instruction::XorLong { dest, a, b } => encode_23x(code, 0xa2, *dest, *a, *b),
        Instruction::ShlLong { dest, a, b } => encode_23x(code, 0xa3, *dest, *a, *b),
        Instruction::ShrLong { dest, a, b } => encode_23x(code, 0xa4, *dest, *a, *b),
        Instruction::UshrLong { dest, a, b } => encode_23x(code, 0xa5, *dest, *a, *b),
        Instruction::AddFloat { dest, a, b } => encode_23x(code, 0xa6, *dest, *a, *b),
        Instruction::SubFloat { dest, a, b } => encode_23x(code, 0xa7, *dest, *a, *b),
        Instruction::MulFloat { dest, a, b } => encode_23x(code, 0xa8, *dest, *a, *b),
        Instruction::DivFloat { dest, a, b } => encode_23x(code, 0xa9, *dest, *a, *b),
        Instruction::RemFloat { dest, a, b } => encode_23x(code, 0xaa, *dest, *a, *b),
        Instruction::AddDouble { dest, a, b } => encode_23x(code, 0xab, *dest, *a, *b),
        Instruction::SubDouble { dest, a, b } => encode_23x(code, 0xac, *dest, *a, *b),
        Instruction::MulDouble { dest, a, b } => encode_23x(code, 0xad, *dest, *a, *b),
        Instruction::DivDouble { dest, a, b } => encode_23x(code, 0xae, *dest, *a, *b),
        Instruction::RemDouble { dest, a, b } => encode_23x(code, 0xaf, *dest, *a, *b),

        // 12x: binary 2addr
        Instruction::AddInt2Addr { dest_a, b } => code.push(pack_12x(0xb0, *dest_a, *b)),
        Instruction::SubInt2Addr { dest_a, b } => code.push(pack_12x(0xb1, *dest_a, *b)),
        Instruction::MulInt2Addr { dest_a, b } => code.push(pack_12x(0xb2, *dest_a, *b)),
        Instruction::DivInt2Addr { dest_a, b } => code.push(pack_12x(0xb3, *dest_a, *b)),
        Instruction::RemInt2Addr { dest_a, b } => code.push(pack_12x(0xb4, *dest_a, *b)),
        Instruction::AndInt2Addr { dest_a, b } => code.push(pack_12x(0xb5, *dest_a, *b)),
        Instruction::OrInt2Addr { dest_a, b } => code.push(pack_12x(0xb6, *dest_a, *b)),
        Instruction::XorInt2Addr { dest_a, b } => code.push(pack_12x(0xb7, *dest_a, *b)),
        Instruction::ShlInt2Addr { dest_a, b } => code.push(pack_12x(0xb8, *dest_a, *b)),
        Instruction::ShrInt2Addr { dest_a, b } => code.push(pack_12x(0xb9, *dest_a, *b)),
        Instruction::UshrInt2Addr { dest_a, b } => code.push(pack_12x(0xba, *dest_a, *b)),
        Instruction::AddLong2Addr { dest_a, b } => code.push(pack_12x(0xbb, *dest_a, *b)),
        Instruction::SubLong2Addr { dest_a, b } => code.push(pack_12x(0xbc, *dest_a, *b)),
        Instruction::MulLong2Addr { dest_a, b } => code.push(pack_12x(0xbd, *dest_a, *b)),
        Instruction::DivLong2Addr { dest_a, b } => code.push(pack_12x(0xbe, *dest_a, *b)),
        Instruction::RemLong2Addr { dest_a, b } => code.push(pack_12x(0xbf, *dest_a, *b)),
        Instruction::AndLong2Addr { dest_a, b } => code.push(pack_12x(0xc0, *dest_a, *b)),
        Instruction::OrLong2Addr { dest_a, b } => code.push(pack_12x(0xc1, *dest_a, *b)),
        Instruction::XorLong2Addr { dest_a, b } => code.push(pack_12x(0xc2, *dest_a, *b)),
        Instruction::ShlLong2Addr { dest_a, b } => code.push(pack_12x(0xc3, *dest_a, *b)),
        Instruction::ShrLong2Addr { dest_a, b } => code.push(pack_12x(0xc4, *dest_a, *b)),
        Instruction::UshrLong2Addr { dest_a, b } => code.push(pack_12x(0xc5, *dest_a, *b)),
        Instruction::AddFloat2Addr { dest_a, b } => code.push(pack_12x(0xc6, *dest_a, *b)),
        Instruction::SubFloat2Addr { dest_a, b } => code.push(pack_12x(0xc7, *dest_a, *b)),
        Instruction::MulFloat2Addr { dest_a, b } => code.push(pack_12x(0xc8, *dest_a, *b)),
        Instruction::DivFloat2Addr { dest_a, b } => code.push(pack_12x(0xc9, *dest_a, *b)),
        Instruction::RemFloat2Addr { dest_a, b } => code.push(pack_12x(0xca, *dest_a, *b)),
        Instruction::AddDouble2Addr { dest_a, b } => code.push(pack_12x(0xcb, *dest_a, *b)),
        Instruction::SubDouble2Addr { dest_a, b } => code.push(pack_12x(0xcc, *dest_a, *b)),
        Instruction::MulDouble2Addr { dest_a, b } => code.push(pack_12x(0xcd, *dest_a, *b)),
        Instruction::DivDouble2Addr { dest_a, b } => code.push(pack_12x(0xce, *dest_a, *b)),
        Instruction::RemDouble2Addr { dest_a, b } => code.push(pack_12x(0xcf, *dest_a, *b)),

        // 22s: binop/lit16
        Instruction::AddIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd0, *dest, *src));
            code.push(*literal as u16);
        }
        Instruction::RsubIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd1, *dest, *src));
            code.push(*literal as u16);
        }
        Instruction::MulIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd2, *dest, *src));
            code.push(*literal as u16);
        }
        Instruction::DivIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd3, *dest, *src));
            code.push(*literal as u16);
        }
        Instruction::RemIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd4, *dest, *src));
            code.push(*literal as u16);
        }
        Instruction::AndIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd5, *dest, *src));
            code.push(*literal as u16);
        }
        Instruction::OrIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd6, *dest, *src));
            code.push(*literal as u16);
        }
        Instruction::XorIntLit16 { dest, src, literal } => {
            code.push(pack_12x(0xd7, *dest, *src));
            code.push(*literal as u16);
        }

        // 22b: binop/lit8
        Instruction::AddIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xd8, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::RsubIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xd9, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::MulIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xda, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::DivIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xdb, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::RemIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xdc, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::AndIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xdd, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::OrIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xde, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::XorIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xdf, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::ShlIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xe0, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::ShrIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xe1, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }
        Instruction::UshrIntLit8 { dest, src, literal } => {
            code.push(pack_aa_op(0xe2, *dest));
            code.push((*src as u16) | ((*literal as u8 as u16) << 8));
        }

        // Payloads
        Instruction::PackedSwitchPayload { first_key, targets } => {
            code.push(0x0100);
            code.push(targets.len() as u16);
            code.push(*first_key as u16);
            code.push((*first_key >> 16) as u16);
            for t in targets {
                code.push(*t as u16);
                code.push((*t >> 16) as u16);
            }
        }
        Instruction::SparseSwitchPayload { keys_and_targets } => {
            code.push(0x0200);
            code.push(keys_and_targets.len() as u16);
            for (k, _) in keys_and_targets {
                code.push(*k as u16);
                code.push((*k >> 16) as u16);
            }
            for (_, t) in keys_and_targets {
                code.push(*t as u16);
                code.push((*t >> 16) as u16);
            }
        }
        Instruction::FillArrayDataPayload {
            element_width,
            data,
        } => {
            code.push(0x0300);
            code.push(*element_width);
            let count = data.len() / *element_width as usize;
            code.push(count as u16);
            code.push((count >> 16) as u16);
            // Pack data bytes into u16 units
            let mut i = 0;
            while i < data.len() {
                let lo = data[i];
                let hi = if i + 1 < data.len() { data[i + 1] } else { 0 };
                code.push(u16::from_le_bytes([lo, hi]));
                i += 2;
            }
        }

        Instruction::RawInstruction { code_units } => {
            code.extend_from_slice(code_units);
        }
    }
    Ok(())
}

fn pack_aa_op(op: u16, aa: u8) -> u16 {
    op | ((aa as u16) << 8)
}

fn pack_12x(op: u16, a: u8, b: u8) -> u16 {
    op | ((a as u16 & 0xF) << 8) | ((b as u16 & 0xF) << 12)
}

fn encode_23x(code: &mut Vec<u16>, op: u16, aa: u8, bb: u8, cc: u8) {
    code.push(op | ((aa as u16) << 8));
    code.push((bb as u16) | ((cc as u16) << 8));
}

fn encode_35c(code: &mut Vec<u16>, op: u16, idx: u16, args: &[u8]) -> Result<()> {
    validate_35c_args(args)?;
    let count = args.len() as u8;
    let (c, d, e, f, g) = unpack_args(args);
    code.push(op | ((count as u16) << 12) | ((g as u16) << 8));
    code.push(idx);
    code.push((c as u16) | ((d as u16) << 4) | ((e as u16) << 8) | ((f as u16) << 12));
    Ok(())
}

fn validate_35c_args(args: &[u8]) -> Result<()> {
    if args.len() > 5 {
        return Err(crate::error::invalid(
            "instruction",
            format!(
                "register count {} exceeds maximum 5 for format 35c/45cc — \
                 use the range variant instead",
                args.len()
            ),
        ));
    }
    if let Some(&r) = args.iter().find(|&&r| r > 15) {
        return Err(crate::error::invalid(
            "instruction",
            format!(
                "register v{r} exceeds nibble range (0-15) for format 35c/45cc — \
                 use the range variant instead"
            ),
        ));
    }
    Ok(())
}

fn unpack_args(args: &[u8]) -> (u8, u8, u8, u8, u8) {
    let c = args.first().copied().unwrap_or(0);
    let d = args.get(1).copied().unwrap_or(0);
    let e = args.get(2).copied().unwrap_or(0);
    let f = args.get(3).copied().unwrap_or(0);
    let g = args.get(4).copied().unwrap_or(0);
    (c, d, e, f, g)
}
