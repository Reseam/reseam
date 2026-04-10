use stitch_apk::stitch_dex::{self, DexFile, FieldIdx, MethodIdx};

use super::types::{
    Branch0Insn, Branch2Insn, BranchInsn, FieldRef, FillArrayInsn, FilledArrayInsn,
    FilledArrayRangeInsn, Instruction, InvokeInsn, InvokeRangeInsn, MethodRef, PackedSwitchInsn,
    Reg1Insn, Reg2Insn, Reg3Insn, RegFieldInsn, RegLiteralInsn, RegStringInsn, RegTypeInsn,
    SimpleInsn, SparseSwitchInsn,
};

fn resolve_method_ref(dex: &DexFile, idx: MethodIdx) -> MethodRef {
    let mid = &dex.methods[idx.0 as usize];
    let class = dex.type_descriptor(mid.class).to_owned();
    let name = dex.string(mid.name).to_owned();
    let proto = &dex.prototypes[mid.proto.0 as usize];
    let ret = dex.type_descriptor(proto.return_type);
    let params: Vec<_> = proto
        .parameters
        .iter()
        .map(|t| dex.type_descriptor(*t))
        .collect();
    let proto_str = format!("({}){}", params.join(""), ret);
    MethodRef {
        defining_class: class,
        name,
        proto: proto_str,
    }
}

fn resolve_field_ref(dex: &DexFile, idx: FieldIdx) -> FieldRef {
    let fid = &dex.fields[idx.0 as usize];
    FieldRef {
        defining_class: dex.type_descriptor(fid.class).to_owned(),
        name: dex.string(fid.name).to_owned(),
        field_type: dex.type_descriptor(fid.type_).to_owned(),
    }
}

pub fn dex_to_kotlin(insn: &stitch_dex::Instruction, dex: &DexFile) -> Instruction {
    use stitch_dex::Instruction as D;
    match insn {
        // Simple (opcode only)
        D::Nop => Instruction::Simple(SimpleInsn { opcode: 0x00 }),
        D::ReturnVoid => Instruction::Simple(SimpleInsn { opcode: 0x0e }),

        // Reg1 (opcode + 1 register)
        D::MoveResult { dest } => Instruction::Reg1(Reg1Insn {
            opcode: 0x0a,
            reg_a: u16::from(*dest),
        }),
        D::MoveResultWide { dest } => Instruction::Reg1(Reg1Insn {
            opcode: 0x0b,
            reg_a: u16::from(*dest),
        }),
        D::MoveResultObject { dest } => Instruction::Reg1(Reg1Insn {
            opcode: 0x0c,
            reg_a: u16::from(*dest),
        }),
        D::MoveException { dest } => Instruction::Reg1(Reg1Insn {
            opcode: 0x0d,
            reg_a: u16::from(*dest),
        }),
        D::Return { src } => Instruction::Reg1(Reg1Insn {
            opcode: 0x0f,
            reg_a: u16::from(*src),
        }),
        D::ReturnWide { src } => Instruction::Reg1(Reg1Insn {
            opcode: 0x10,
            reg_a: u16::from(*src),
        }),
        D::ReturnObject { src } => Instruction::Reg1(Reg1Insn {
            opcode: 0x11,
            reg_a: u16::from(*src),
        }),
        D::MonitorEnter { ref_ } => Instruction::Reg1(Reg1Insn {
            opcode: 0x1d,
            reg_a: u16::from(*ref_),
        }),
        D::MonitorExit { ref_ } => Instruction::Reg1(Reg1Insn {
            opcode: 0x1e,
            reg_a: u16::from(*ref_),
        }),
        D::Throw { exception } => Instruction::Reg1(Reg1Insn {
            opcode: 0x27,
            reg_a: u16::from(*exception),
        }),

        // Reg2 (opcode + 2 registers)
        D::Move { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x01,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::MoveFrom16 { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x02,
            reg_a: u16::from(*dest),
            reg_b: *src,
        }),
        D::Move16 { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x03,
            reg_a: *dest,
            reg_b: *src,
        }),
        D::MoveWide { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x04,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::MoveWideFrom16 { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x05,
            reg_a: u16::from(*dest),
            reg_b: *src,
        }),
        D::MoveWide16 { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x06,
            reg_a: *dest,
            reg_b: *src,
        }),
        D::MoveObject { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x07,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::MoveObjectFrom16 { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x08,
            reg_a: u16::from(*dest),
            reg_b: *src,
        }),
        D::MoveObject16 { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x09,
            reg_a: *dest,
            reg_b: *src,
        }),
        D::ArrayLength { dest, array } => Instruction::Reg2(Reg2Insn {
            opcode: 0x21,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*array),
        }),
        D::NegInt { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x7b,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::NotInt { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x7c,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::NegLong { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x7d,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::NotLong { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x7e,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::NegFloat { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x7f,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::NegDouble { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x80,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::IntToLong { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x81,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::IntToFloat { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x82,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::IntToDouble { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x83,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::LongToInt { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x84,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::LongToFloat { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x85,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::LongToDouble { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x86,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::FloatToInt { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x87,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::FloatToLong { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x88,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::FloatToDouble { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x89,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::DoubleToInt { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x8a,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::DoubleToLong { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x8b,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::DoubleToFloat { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x8c,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::IntToByte { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x8d,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::IntToChar { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x8e,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::IntToShort { dest, src } => Instruction::Reg2(Reg2Insn {
            opcode: 0x8f,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
        }),
        D::AddInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb0,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::SubInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb1,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::MulInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb2,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::DivInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb3,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::RemInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb4,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::AndInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb5,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::OrInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb6,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::XorInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb7,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::ShlInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb8,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::ShrInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xb9,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::UshrInt2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xba,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::AddLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xbb,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::SubLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xbc,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::MulLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xbd,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::DivLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xbe,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::RemLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xbf,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::AndLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc0,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::OrLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc1,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::XorLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc2,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::ShlLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc3,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::ShrLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc4,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::UshrLong2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc5,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::AddFloat2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc6,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::SubFloat2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc7,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::MulFloat2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc8,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::DivFloat2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xc9,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::RemFloat2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xca,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::AddDouble2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xcb,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::SubDouble2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xcc,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::MulDouble2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xcd,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::DivDouble2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xce,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),
        D::RemDouble2Addr { dest_a, b } => Instruction::Reg2(Reg2Insn {
            opcode: 0xcf,
            reg_a: u16::from(*dest_a),
            reg_b: u16::from(*b),
        }),

        // Reg3 (opcode + 3 registers)
        D::CmpLFloat { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x2d,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::CmpGFloat { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x2e,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::CmpLDouble { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x2f,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::CmpGDouble { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x30,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::CmpLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x31,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::Aget { dest, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x44,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AgetWide { dest, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x45,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AgetObject { dest, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x46,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AgetBoolean { dest, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x47,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AgetByte { dest, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x48,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AgetChar { dest, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x49,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AgetShort { dest, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x4a,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::Aput { src, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x4b,
            reg_a: u16::from(*src),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AputWide { src, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x4c,
            reg_a: u16::from(*src),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AputObject { src, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x4d,
            reg_a: u16::from(*src),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AputBoolean { src, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x4e,
            reg_a: u16::from(*src),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AputByte { src, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x4f,
            reg_a: u16::from(*src),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AputChar { src, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x50,
            reg_a: u16::from(*src),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AputShort { src, array, index } => Instruction::Reg3(Reg3Insn {
            opcode: 0x51,
            reg_a: u16::from(*src),
            reg_b: u16::from(*array),
            reg_c: u16::from(*index),
        }),
        D::AddInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x90,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::SubInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x91,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::MulInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x92,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::DivInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x93,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::RemInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x94,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::AndInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x95,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::OrInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x96,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::XorInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x97,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::ShlInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x98,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::ShrInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x99,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::UshrInt { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x9a,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::AddLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x9b,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::SubLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x9c,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::MulLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x9d,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::DivLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x9e,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::RemLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0x9f,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::AndLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa0,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::OrLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa1,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::XorLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa2,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::ShlLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa3,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::ShrLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa4,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::UshrLong { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa5,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::AddFloat { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa6,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::SubFloat { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa7,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::MulFloat { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa8,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::DivFloat { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xa9,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::RemFloat { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xaa,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::AddDouble { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xab,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::SubDouble { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xac,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::MulDouble { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xad,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::DivDouble { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xae,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),
        D::RemDouble { dest, a, b } => Instruction::Reg3(Reg3Insn {
            opcode: 0xaf,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*a),
            reg_c: u16::from(*b),
        }),

        // RegLiteral (opcode + reg(s) + literal value)
        D::Const4 { dest, value } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0x12,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: i64::from(*value),
        }),
        D::Const16 { dest, value } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0x13,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: i64::from(*value),
        }),
        D::Const { dest, value } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0x14,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: i64::from(*value),
        }),
        D::ConstHigh16 { dest, value } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0x15,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: i64::from(*value),
        }),
        D::ConstWide16 { dest, value } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0x16,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: i64::from(*value),
        }),
        D::ConstWide32 { dest, value } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0x17,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: i64::from(*value),
        }),
        D::ConstWide { dest, value } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0x18,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: *value,
        }),
        D::ConstWideHigh16 { dest, value } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0x19,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: i64::from(*value),
        }),
        D::AddIntLit16 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd0,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::RsubIntLit16 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd1,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::MulIntLit16 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd2,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::DivIntLit16 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd3,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::RemIntLit16 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd4,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::AndIntLit16 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd5,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::OrIntLit16 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd6,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::XorIntLit16 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd7,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::AddIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd8,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::RsubIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xd9,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::MulIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xda,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::DivIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xdb,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::RemIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xdc,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::AndIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xdd,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::OrIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xde,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::XorIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xdf,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::ShlIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xe0,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::ShrIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xe1,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),
        D::UshrIntLit8 { dest, src, literal } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xe2,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*src),
            literal: i64::from(*literal),
        }),

        // RegString (opcode + reg + string)
        D::ConstString { dest, string } => Instruction::RegString(RegStringInsn {
            opcode: 0x1a,
            reg_a: u16::from(*dest),
            value: dex.string(*string).to_owned(),
        }),
        D::ConstStringJumbo { dest, string } => Instruction::RegString(RegStringInsn {
            opcode: 0x1b,
            reg_a: u16::from(*dest),
            value: dex.string(*string).to_owned(),
        }),

        // RegType (opcode + reg(s) + type descriptor)
        D::ConstClass { dest, type_ } => Instruction::RegType(RegTypeInsn {
            opcode: 0x1c,
            reg_a: u16::from(*dest),
            reg_b: 0,
            type_descriptor: dex.type_descriptor(*type_).to_owned(),
        }),
        D::CheckCast { ref_, type_ } => Instruction::RegType(RegTypeInsn {
            opcode: 0x1f,
            reg_a: u16::from(*ref_),
            reg_b: 0,
            type_descriptor: dex.type_descriptor(*type_).to_owned(),
        }),
        D::InstanceOf { dest, ref_, type_ } => Instruction::RegType(RegTypeInsn {
            opcode: 0x20,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*ref_),
            type_descriptor: dex.type_descriptor(*type_).to_owned(),
        }),
        D::NewInstance { dest, type_ } => Instruction::RegType(RegTypeInsn {
            opcode: 0x22,
            reg_a: u16::from(*dest),
            reg_b: 0,
            type_descriptor: dex.type_descriptor(*type_).to_owned(),
        }),
        D::NewArray { dest, size, type_ } => Instruction::RegType(RegTypeInsn {
            opcode: 0x23,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*size),
            type_descriptor: dex.type_descriptor(*type_).to_owned(),
        }),

        // RegField (opcode + reg(s) + field reference)
        D::Iget { dest, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x52,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IgetWide { dest, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x53,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IgetObject { dest, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x54,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IgetBoolean { dest, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x55,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IgetByte { dest, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x56,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IgetChar { dest, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x57,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IgetShort { dest, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x58,
            reg_a: u16::from(*dest),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::Iput { src, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x59,
            reg_a: u16::from(*src),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IputWide { src, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x5a,
            reg_a: u16::from(*src),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IputObject { src, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x5b,
            reg_a: u16::from(*src),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IputBoolean { src, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x5c,
            reg_a: u16::from(*src),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IputByte { src, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x5d,
            reg_a: u16::from(*src),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IputChar { src, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x5e,
            reg_a: u16::from(*src),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::IputShort { src, obj, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x5f,
            reg_a: u16::from(*src),
            reg_b: u16::from(*obj),
            field: resolve_field_ref(dex, *field),
        }),
        D::Sget { dest, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x60,
            reg_a: u16::from(*dest),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SgetWide { dest, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x61,
            reg_a: u16::from(*dest),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SgetObject { dest, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x62,
            reg_a: u16::from(*dest),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SgetBoolean { dest, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x63,
            reg_a: u16::from(*dest),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SgetByte { dest, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x64,
            reg_a: u16::from(*dest),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SgetChar { dest, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x65,
            reg_a: u16::from(*dest),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SgetShort { dest, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x66,
            reg_a: u16::from(*dest),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::Sput { src, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x67,
            reg_a: u16::from(*src),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SputWide { src, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x68,
            reg_a: u16::from(*src),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SputObject { src, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x69,
            reg_a: u16::from(*src),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SputBoolean { src, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x6a,
            reg_a: u16::from(*src),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SputByte { src, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x6b,
            reg_a: u16::from(*src),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SputChar { src, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x6c,
            reg_a: u16::from(*src),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),
        D::SputShort { src, field } => Instruction::RegField(RegFieldInsn {
            opcode: 0x6d,
            reg_a: u16::from(*src),
            reg_b: 0,
            field: resolve_field_ref(dex, *field),
        }),

        // Invoke (opcode + register list + method reference)
        D::InvokeVirtual { method, args } => Instruction::Invoke(InvokeInsn {
            opcode: 0x6e,
            registers: args.iter().map(|r| u16::from(*r)).collect(),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeSuper { method, args } => Instruction::Invoke(InvokeInsn {
            opcode: 0x6f,
            registers: args.iter().map(|r| u16::from(*r)).collect(),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeDirect { method, args } => Instruction::Invoke(InvokeInsn {
            opcode: 0x70,
            registers: args.iter().map(|r| u16::from(*r)).collect(),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeStatic { method, args } => Instruction::Invoke(InvokeInsn {
            opcode: 0x71,
            registers: args.iter().map(|r| u16::from(*r)).collect(),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeInterface { method, args } => Instruction::Invoke(InvokeInsn {
            opcode: 0x72,
            registers: args.iter().map(|r| u16::from(*r)).collect(),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokePolymorphic {
            method,
            proto: _,
            args,
        } => Instruction::Invoke(InvokeInsn {
            opcode: 0xfa,
            registers: args.iter().map(|r| u16::from(*r)).collect(),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeCustom { call_site, args } => Instruction::Invoke(InvokeInsn {
            opcode: 0xfc,
            registers: args.iter().map(|r| u16::from(*r)).collect(),
            method: MethodRef {
                defining_class: String::new(),
                name: format!("call_site_{}", call_site.0),
                proto: String::new(),
            },
        }),

        // InvokeRange (opcode + start_reg + count + method reference)
        D::InvokeVirtualRange {
            method,
            first_reg,
            count,
        } => Instruction::InvokeRange(InvokeRangeInsn {
            opcode: 0x74,
            start_reg: *first_reg,
            reg_count: u16::from(*count),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeSuperRange {
            method,
            first_reg,
            count,
        } => Instruction::InvokeRange(InvokeRangeInsn {
            opcode: 0x75,
            start_reg: *first_reg,
            reg_count: u16::from(*count),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeDirectRange {
            method,
            first_reg,
            count,
        } => Instruction::InvokeRange(InvokeRangeInsn {
            opcode: 0x76,
            start_reg: *first_reg,
            reg_count: u16::from(*count),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeStaticRange {
            method,
            first_reg,
            count,
        } => Instruction::InvokeRange(InvokeRangeInsn {
            opcode: 0x77,
            start_reg: *first_reg,
            reg_count: u16::from(*count),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeInterfaceRange {
            method,
            first_reg,
            count,
        } => Instruction::InvokeRange(InvokeRangeInsn {
            opcode: 0x78,
            start_reg: *first_reg,
            reg_count: u16::from(*count),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokePolymorphicRange {
            method,
            proto: _,
            first_reg,
            count,
        } => Instruction::InvokeRange(InvokeRangeInsn {
            opcode: 0xfb,
            start_reg: *first_reg,
            reg_count: u16::from(*count),
            method: resolve_method_ref(dex, *method),
        }),
        D::InvokeCustomRange {
            call_site,
            first_reg,
            count,
        } => Instruction::InvokeRange(InvokeRangeInsn {
            opcode: 0xfd,
            start_reg: *first_reg,
            reg_count: u16::from(*count),
            method: MethodRef {
                defining_class: String::new(),
                name: format!("call_site_{}", call_site.0),
                proto: String::new(),
            },
        }),

        // ConstMethodHandle / ConstMethodType → RegLiteral (opcode + dest + index as literal)
        D::ConstMethodHandle {
            dest,
            method_handle,
        } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xfe,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: i64::from(method_handle.0),
        }),
        D::ConstMethodType { dest, proto } => Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0xff,
            reg_a: u16::from(*dest),
            reg_b: 0,
            literal: i64::from(proto.0),
        }),

        // Branch0 (opcode + offset, no registers)
        D::Goto { offset } => Instruction::Branch0(Branch0Insn {
            opcode: 0x28,
            offset: i32::from(*offset),
        }),
        D::Goto16 { offset } => Instruction::Branch0(Branch0Insn {
            opcode: 0x29,
            offset: i32::from(*offset),
        }),
        D::Goto32 { offset } => Instruction::Branch0(Branch0Insn {
            opcode: 0x2a,
            offset: *offset,
        }),

        // Branch (opcode + 1 register + offset)
        D::IfEqz { a, offset } => Instruction::Branch(BranchInsn {
            opcode: 0x38,
            reg_a: u16::from(*a),
            offset: i32::from(*offset),
        }),
        D::IfNez { a, offset } => Instruction::Branch(BranchInsn {
            opcode: 0x39,
            reg_a: u16::from(*a),
            offset: i32::from(*offset),
        }),
        D::IfLtz { a, offset } => Instruction::Branch(BranchInsn {
            opcode: 0x3a,
            reg_a: u16::from(*a),
            offset: i32::from(*offset),
        }),
        D::IfGez { a, offset } => Instruction::Branch(BranchInsn {
            opcode: 0x3b,
            reg_a: u16::from(*a),
            offset: i32::from(*offset),
        }),
        D::IfGtz { a, offset } => Instruction::Branch(BranchInsn {
            opcode: 0x3c,
            reg_a: u16::from(*a),
            offset: i32::from(*offset),
        }),
        D::IfLez { a, offset } => Instruction::Branch(BranchInsn {
            opcode: 0x3d,
            reg_a: u16::from(*a),
            offset: i32::from(*offset),
        }),
        D::PackedSwitch {
            test,
            payload_offset,
        } => Instruction::Branch(BranchInsn {
            opcode: 0x2b,
            reg_a: u16::from(*test),
            offset: *payload_offset,
        }),
        D::SparseSwitch {
            test,
            payload_offset,
        } => Instruction::Branch(BranchInsn {
            opcode: 0x2c,
            reg_a: u16::from(*test),
            offset: *payload_offset,
        }),
        D::FillArrayData {
            array,
            payload_offset,
        } => Instruction::Branch(BranchInsn {
            opcode: 0x26,
            reg_a: u16::from(*array),
            offset: *payload_offset,
        }),

        // Branch2 (opcode + 2 registers + offset)
        D::IfEq { a, b, offset } => Instruction::Branch2(Branch2Insn {
            opcode: 0x32,
            reg_a: u16::from(*a),
            reg_b: u16::from(*b),
            offset: i32::from(*offset),
        }),
        D::IfNe { a, b, offset } => Instruction::Branch2(Branch2Insn {
            opcode: 0x33,
            reg_a: u16::from(*a),
            reg_b: u16::from(*b),
            offset: i32::from(*offset),
        }),
        D::IfLt { a, b, offset } => Instruction::Branch2(Branch2Insn {
            opcode: 0x34,
            reg_a: u16::from(*a),
            reg_b: u16::from(*b),
            offset: i32::from(*offset),
        }),
        D::IfGe { a, b, offset } => Instruction::Branch2(Branch2Insn {
            opcode: 0x35,
            reg_a: u16::from(*a),
            reg_b: u16::from(*b),
            offset: i32::from(*offset),
        }),
        D::IfGt { a, b, offset } => Instruction::Branch2(Branch2Insn {
            opcode: 0x36,
            reg_a: u16::from(*a),
            reg_b: u16::from(*b),
            offset: i32::from(*offset),
        }),
        D::IfLe { a, b, offset } => Instruction::Branch2(Branch2Insn {
            opcode: 0x37,
            reg_a: u16::from(*a),
            reg_b: u16::from(*b),
            offset: i32::from(*offset),
        }),

        // FilledArray
        D::FilledNewArray { type_, args } => Instruction::FilledArray(FilledArrayInsn {
            opcode: 0x24,
            registers: args.iter().map(|r| u16::from(*r)).collect(),
            type_descriptor: dex.type_descriptor(*type_).to_owned(),
        }),

        // FilledArrayRange
        D::FilledNewArrayRange {
            type_,
            first_reg,
            count,
        } => Instruction::FilledArrayRange(FilledArrayRangeInsn {
            opcode: 0x25,
            start_reg: *first_reg,
            reg_count: u16::from(*count),
            type_descriptor: dex.type_descriptor(*type_).to_owned(),
        }),

        // Payload data
        D::PackedSwitchPayload { first_key, targets } => {
            Instruction::PackedSwitchData(PackedSwitchInsn {
                first_key: *first_key,
                targets: targets.clone(),
            })
        }
        D::SparseSwitchPayload { keys_and_targets } => {
            Instruction::SparseSwitchData(SparseSwitchInsn {
                keys: keys_and_targets.iter().map(|(k, _)| *k).collect(),
                targets: keys_and_targets.iter().map(|(_, t)| *t).collect(),
            })
        }
        D::FillArrayDataPayload {
            element_width,
            data,
        } => Instruction::FillArrayData(FillArrayInsn {
            element_width: *element_width,
            data: data.clone(),
        }),

        // Raw
        D::RawInstruction { code_units } => {
            let bytes: Vec<u8> = code_units.iter().flat_map(|u| u.to_le_bytes()).collect();
            Instruction::Raw(bytes)
        }

        _ => {
            let bytes = insn
                .opcode()
                .map_or_else(Vec::new, |op| op.to_le_bytes().to_vec());
            Instruction::Raw(bytes)
        }
    }
}

pub fn kotlin_to_dex(insn: &Instruction, dex: &mut DexFile) -> stitch_dex::Instruction {
    use stitch_dex::Instruction as D;
    match insn {
        Instruction::Simple(s) => match s.opcode {
            0x00 => D::Nop,
            0x0e => D::ReturnVoid,
            _ => raw_from_opcode(s.opcode),
        },
        Instruction::Reg1(r) => {
            let a = r.reg_a as u8;
            match r.opcode {
                0x0a => D::MoveResult { dest: a },
                0x0b => D::MoveResultWide { dest: a },
                0x0c => D::MoveResultObject { dest: a },
                0x0d => D::MoveException { dest: a },
                0x0f => D::Return { src: a },
                0x10 => D::ReturnWide { src: a },
                0x11 => D::ReturnObject { src: a },
                0x1d => D::MonitorEnter { ref_: a },
                0x1e => D::MonitorExit { ref_: a },
                0x27 => D::Throw { exception: a },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::Reg2(r) => {
            let a = r.reg_a;
            let b = r.reg_b;
            match r.opcode {
                0x01 => D::Move {
                    dest: a as u8,
                    src: b as u8,
                },
                0x02 => D::MoveFrom16 {
                    dest: a as u8,
                    src: b,
                },
                0x03 => D::Move16 { dest: a, src: b },
                0x04 => D::MoveWide {
                    dest: a as u8,
                    src: b as u8,
                },
                0x05 => D::MoveWideFrom16 {
                    dest: a as u8,
                    src: b,
                },
                0x06 => D::MoveWide16 { dest: a, src: b },
                0x07 => D::MoveObject {
                    dest: a as u8,
                    src: b as u8,
                },
                0x08 => D::MoveObjectFrom16 {
                    dest: a as u8,
                    src: b,
                },
                0x09 => D::MoveObject16 { dest: a, src: b },
                0x21 => D::ArrayLength {
                    dest: a as u8,
                    array: b as u8,
                },
                0x7b => D::NegInt {
                    dest: a as u8,
                    src: b as u8,
                },
                0x7c => D::NotInt {
                    dest: a as u8,
                    src: b as u8,
                },
                0x7d => D::NegLong {
                    dest: a as u8,
                    src: b as u8,
                },
                0x7e => D::NotLong {
                    dest: a as u8,
                    src: b as u8,
                },
                0x7f => D::NegFloat {
                    dest: a as u8,
                    src: b as u8,
                },
                0x80 => D::NegDouble {
                    dest: a as u8,
                    src: b as u8,
                },
                0x81 => D::IntToLong {
                    dest: a as u8,
                    src: b as u8,
                },
                0x82 => D::IntToFloat {
                    dest: a as u8,
                    src: b as u8,
                },
                0x83 => D::IntToDouble {
                    dest: a as u8,
                    src: b as u8,
                },
                0x84 => D::LongToInt {
                    dest: a as u8,
                    src: b as u8,
                },
                0x85 => D::LongToFloat {
                    dest: a as u8,
                    src: b as u8,
                },
                0x86 => D::LongToDouble {
                    dest: a as u8,
                    src: b as u8,
                },
                0x87 => D::FloatToInt {
                    dest: a as u8,
                    src: b as u8,
                },
                0x88 => D::FloatToLong {
                    dest: a as u8,
                    src: b as u8,
                },
                0x89 => D::FloatToDouble {
                    dest: a as u8,
                    src: b as u8,
                },
                0x8a => D::DoubleToInt {
                    dest: a as u8,
                    src: b as u8,
                },
                0x8b => D::DoubleToLong {
                    dest: a as u8,
                    src: b as u8,
                },
                0x8c => D::DoubleToFloat {
                    dest: a as u8,
                    src: b as u8,
                },
                0x8d => D::IntToByte {
                    dest: a as u8,
                    src: b as u8,
                },
                0x8e => D::IntToChar {
                    dest: a as u8,
                    src: b as u8,
                },
                0x8f => D::IntToShort {
                    dest: a as u8,
                    src: b as u8,
                },
                0xb0 => D::AddInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xb1 => D::SubInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xb2 => D::MulInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xb3 => D::DivInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xb4 => D::RemInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xb5 => D::AndInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xb6 => D::OrInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xb7 => D::XorInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xb8 => D::ShlInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xb9 => D::ShrInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xba => D::UshrInt2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xbb => D::AddLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xbc => D::SubLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xbd => D::MulLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xbe => D::DivLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xbf => D::RemLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc0 => D::AndLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc1 => D::OrLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc2 => D::XorLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc3 => D::ShlLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc4 => D::ShrLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc5 => D::UshrLong2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc6 => D::AddFloat2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc7 => D::SubFloat2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc8 => D::MulFloat2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xc9 => D::DivFloat2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xca => D::RemFloat2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xcb => D::AddDouble2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xcc => D::SubDouble2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xcd => D::MulDouble2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xce => D::DivDouble2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                0xcf => D::RemDouble2Addr {
                    dest_a: a as u8,
                    b: b as u8,
                },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::Reg3(r) => {
            let (a, b, c) = (r.reg_a as u8, r.reg_b as u8, r.reg_c as u8);
            match r.opcode {
                0x2d => D::CmpLFloat {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x2e => D::CmpGFloat {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x2f => D::CmpLDouble {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x30 => D::CmpGDouble {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x31 => D::CmpLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x44 => D::Aget {
                    dest: a,
                    array: b,
                    index: c,
                },
                0x45 => D::AgetWide {
                    dest: a,
                    array: b,
                    index: c,
                },
                0x46 => D::AgetObject {
                    dest: a,
                    array: b,
                    index: c,
                },
                0x47 => D::AgetBoolean {
                    dest: a,
                    array: b,
                    index: c,
                },
                0x48 => D::AgetByte {
                    dest: a,
                    array: b,
                    index: c,
                },
                0x49 => D::AgetChar {
                    dest: a,
                    array: b,
                    index: c,
                },
                0x4a => D::AgetShort {
                    dest: a,
                    array: b,
                    index: c,
                },
                0x4b => D::Aput {
                    src: a,
                    array: b,
                    index: c,
                },
                0x4c => D::AputWide {
                    src: a,
                    array: b,
                    index: c,
                },
                0x4d => D::AputObject {
                    src: a,
                    array: b,
                    index: c,
                },
                0x4e => D::AputBoolean {
                    src: a,
                    array: b,
                    index: c,
                },
                0x4f => D::AputByte {
                    src: a,
                    array: b,
                    index: c,
                },
                0x50 => D::AputChar {
                    src: a,
                    array: b,
                    index: c,
                },
                0x51 => D::AputShort {
                    src: a,
                    array: b,
                    index: c,
                },
                0x90 => D::AddInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x91 => D::SubInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x92 => D::MulInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x93 => D::DivInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x94 => D::RemInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x95 => D::AndInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x96 => D::OrInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x97 => D::XorInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x98 => D::ShlInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x99 => D::ShrInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x9a => D::UshrInt {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x9b => D::AddLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x9c => D::SubLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x9d => D::MulLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x9e => D::DivLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0x9f => D::RemLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa0 => D::AndLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa1 => D::OrLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa2 => D::XorLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa3 => D::ShlLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa4 => D::ShrLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa5 => D::UshrLong {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa6 => D::AddFloat {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa7 => D::SubFloat {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa8 => D::MulFloat {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xa9 => D::DivFloat {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xaa => D::RemFloat {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xab => D::AddDouble {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xac => D::SubDouble {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xad => D::MulDouble {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xae => D::DivDouble {
                    dest: a,
                    a: b,
                    b: c,
                },
                0xaf => D::RemDouble {
                    dest: a,
                    a: b,
                    b: c,
                },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::RegLiteral(r) => {
            let (a, b) = (r.reg_a as u8, r.reg_b as u8);
            match r.opcode {
                0x12 => D::Const4 {
                    dest: a,
                    value: r.literal as i8,
                },
                0x13 => D::Const16 {
                    dest: a,
                    value: r.literal as i16,
                },
                0x14 => D::Const {
                    dest: a,
                    value: r.literal as i32,
                },
                0x15 => D::ConstHigh16 {
                    dest: a,
                    value: r.literal as i16,
                },
                0x16 => D::ConstWide16 {
                    dest: a,
                    value: r.literal as i16,
                },
                0x17 => D::ConstWide32 {
                    dest: a,
                    value: r.literal as i32,
                },
                0x18 => D::ConstWide {
                    dest: a,
                    value: r.literal,
                },
                0x19 => D::ConstWideHigh16 {
                    dest: a,
                    value: r.literal as i16,
                },
                0xd0 => D::AddIntLit16 {
                    dest: a,
                    src: b,
                    literal: r.literal as i16,
                },
                0xd1 => D::RsubIntLit16 {
                    dest: a,
                    src: b,
                    literal: r.literal as i16,
                },
                0xd2 => D::MulIntLit16 {
                    dest: a,
                    src: b,
                    literal: r.literal as i16,
                },
                0xd3 => D::DivIntLit16 {
                    dest: a,
                    src: b,
                    literal: r.literal as i16,
                },
                0xd4 => D::RemIntLit16 {
                    dest: a,
                    src: b,
                    literal: r.literal as i16,
                },
                0xd5 => D::AndIntLit16 {
                    dest: a,
                    src: b,
                    literal: r.literal as i16,
                },
                0xd6 => D::OrIntLit16 {
                    dest: a,
                    src: b,
                    literal: r.literal as i16,
                },
                0xd7 => D::XorIntLit16 {
                    dest: a,
                    src: b,
                    literal: r.literal as i16,
                },
                0xd8 => D::AddIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xd9 => D::RsubIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xda => D::MulIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xdb => D::DivIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xdc => D::RemIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xdd => D::AndIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xde => D::OrIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xdf => D::XorIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xe0 => D::ShlIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xe1 => D::ShrIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xe2 => D::UshrIntLit8 {
                    dest: a,
                    src: b,
                    literal: r.literal as i8,
                },
                0xfe => D::ConstMethodHandle {
                    dest: a,
                    method_handle: stitch_dex::MethodHandleIdx(r.literal as u32),
                },
                0xff => D::ConstMethodType {
                    dest: a,
                    proto: stitch_dex::ProtoIdx(r.literal as u16),
                },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::RegString(r) => {
            let string = dex.intern_string(&r.value);
            match r.opcode {
                0x1a => D::ConstString {
                    dest: r.reg_a as u8,
                    string,
                },
                0x1b => D::ConstStringJumbo {
                    dest: r.reg_a as u8,
                    string,
                },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::RegType(r) => {
            let type_ = dex.intern_type(&r.type_descriptor);
            match r.opcode {
                0x1c => D::ConstClass {
                    dest: r.reg_a as u8,
                    type_,
                },
                0x1f => D::CheckCast {
                    ref_: r.reg_a as u8,
                    type_,
                },
                0x20 => D::InstanceOf {
                    dest: r.reg_a as u8,
                    ref_: r.reg_b as u8,
                    type_,
                },
                0x22 => D::NewInstance {
                    dest: r.reg_a as u8,
                    type_,
                },
                0x23 => D::NewArray {
                    dest: r.reg_a as u8,
                    size: r.reg_b as u8,
                    type_,
                },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::RegField(r) => {
            let field = intern_field(dex, &r.field);
            let (a, b) = (r.reg_a as u8, r.reg_b as u8);
            match r.opcode {
                0x52 => D::Iget {
                    dest: a,
                    obj: b,
                    field,
                },
                0x53 => D::IgetWide {
                    dest: a,
                    obj: b,
                    field,
                },
                0x54 => D::IgetObject {
                    dest: a,
                    obj: b,
                    field,
                },
                0x55 => D::IgetBoolean {
                    dest: a,
                    obj: b,
                    field,
                },
                0x56 => D::IgetByte {
                    dest: a,
                    obj: b,
                    field,
                },
                0x57 => D::IgetChar {
                    dest: a,
                    obj: b,
                    field,
                },
                0x58 => D::IgetShort {
                    dest: a,
                    obj: b,
                    field,
                },
                0x59 => D::Iput {
                    src: a,
                    obj: b,
                    field,
                },
                0x5a => D::IputWide {
                    src: a,
                    obj: b,
                    field,
                },
                0x5b => D::IputObject {
                    src: a,
                    obj: b,
                    field,
                },
                0x5c => D::IputBoolean {
                    src: a,
                    obj: b,
                    field,
                },
                0x5d => D::IputByte {
                    src: a,
                    obj: b,
                    field,
                },
                0x5e => D::IputChar {
                    src: a,
                    obj: b,
                    field,
                },
                0x5f => D::IputShort {
                    src: a,
                    obj: b,
                    field,
                },
                0x60 => D::Sget { dest: a, field },
                0x61 => D::SgetWide { dest: a, field },
                0x62 => D::SgetObject { dest: a, field },
                0x63 => D::SgetBoolean { dest: a, field },
                0x64 => D::SgetByte { dest: a, field },
                0x65 => D::SgetChar { dest: a, field },
                0x66 => D::SgetShort { dest: a, field },
                0x67 => D::Sput { src: a, field },
                0x68 => D::SputWide { src: a, field },
                0x69 => D::SputObject { src: a, field },
                0x6a => D::SputBoolean { src: a, field },
                0x6b => D::SputByte { src: a, field },
                0x6c => D::SputChar { src: a, field },
                0x6d => D::SputShort { src: a, field },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::Invoke(r) => {
            let method = intern_method(dex, &r.method);
            let needs_range = r.registers.len() > 5 || r.registers.iter().any(|&reg| reg > 15);
            if needs_range {
                let first_reg = r.registers.first().copied().unwrap_or(0);
                let count = r.registers.len() as u8;
                match r.opcode {
                    0x6e => D::InvokeVirtualRange {
                        method,
                        first_reg,
                        count,
                    },
                    0x6f => D::InvokeSuperRange {
                        method,
                        first_reg,
                        count,
                    },
                    0x70 => D::InvokeDirectRange {
                        method,
                        first_reg,
                        count,
                    },
                    0x71 => D::InvokeStaticRange {
                        method,
                        first_reg,
                        count,
                    },
                    0x72 => D::InvokeInterfaceRange {
                        method,
                        first_reg,
                        count,
                    },
                    _ => raw_from_opcode(r.opcode),
                }
            } else {
                let args = r.registers.iter().map(|r| *r as u8).collect();
                match r.opcode {
                    0x6e => D::InvokeVirtual { method, args },
                    0x6f => D::InvokeSuper { method, args },
                    0x70 => D::InvokeDirect { method, args },
                    0x71 => D::InvokeStatic { method, args },
                    0x72 => D::InvokeInterface { method, args },
                    _ => raw_from_opcode(r.opcode),
                }
            }
        }
        Instruction::InvokeRange(r) => {
            let method = intern_method(dex, &r.method);
            match r.opcode {
                0x74 => D::InvokeVirtualRange {
                    method,
                    first_reg: r.start_reg,
                    count: r.reg_count as u8,
                },
                0x75 => D::InvokeSuperRange {
                    method,
                    first_reg: r.start_reg,
                    count: r.reg_count as u8,
                },
                0x76 => D::InvokeDirectRange {
                    method,
                    first_reg: r.start_reg,
                    count: r.reg_count as u8,
                },
                0x77 => D::InvokeStaticRange {
                    method,
                    first_reg: r.start_reg,
                    count: r.reg_count as u8,
                },
                0x78 => D::InvokeInterfaceRange {
                    method,
                    first_reg: r.start_reg,
                    count: r.reg_count as u8,
                },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::Branch0(r) => match r.opcode {
            0x28 => D::Goto {
                offset: r.offset as i8,
            },
            0x29 => D::Goto16 {
                offset: r.offset as i16,
            },
            0x2a => D::Goto32 { offset: r.offset },
            _ => raw_from_opcode(r.opcode),
        },
        Instruction::Branch(r) => {
            let a = r.reg_a as u8;
            match r.opcode {
                0x26 => D::FillArrayData {
                    array: a,
                    payload_offset: r.offset,
                },
                0x2b => D::PackedSwitch {
                    test: a,
                    payload_offset: r.offset,
                },
                0x2c => D::SparseSwitch {
                    test: a,
                    payload_offset: r.offset,
                },
                0x38 => D::IfEqz {
                    a,
                    offset: r.offset as i16,
                },
                0x39 => D::IfNez {
                    a,
                    offset: r.offset as i16,
                },
                0x3a => D::IfLtz {
                    a,
                    offset: r.offset as i16,
                },
                0x3b => D::IfGez {
                    a,
                    offset: r.offset as i16,
                },
                0x3c => D::IfGtz {
                    a,
                    offset: r.offset as i16,
                },
                0x3d => D::IfLez {
                    a,
                    offset: r.offset as i16,
                },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::Branch2(r) => {
            let (a, b) = (r.reg_a as u8, r.reg_b as u8);
            match r.opcode {
                0x32 => D::IfEq {
                    a,
                    b,
                    offset: r.offset as i16,
                },
                0x33 => D::IfNe {
                    a,
                    b,
                    offset: r.offset as i16,
                },
                0x34 => D::IfLt {
                    a,
                    b,
                    offset: r.offset as i16,
                },
                0x35 => D::IfGe {
                    a,
                    b,
                    offset: r.offset as i16,
                },
                0x36 => D::IfGt {
                    a,
                    b,
                    offset: r.offset as i16,
                },
                0x37 => D::IfLe {
                    a,
                    b,
                    offset: r.offset as i16,
                },
                _ => raw_from_opcode(r.opcode),
            }
        }
        Instruction::FilledArray(r) => {
            let type_ = dex.intern_type(&r.type_descriptor);
            D::FilledNewArray {
                type_,
                args: r.registers.iter().map(|r| *r as u8).collect(),
            }
        }
        Instruction::FilledArrayRange(r) => {
            let type_ = dex.intern_type(&r.type_descriptor);
            D::FilledNewArrayRange {
                type_,
                first_reg: r.start_reg,
                count: r.reg_count as u8,
            }
        }
        Instruction::PackedSwitchData(r) => D::PackedSwitchPayload {
            first_key: r.first_key,
            targets: r.targets.clone(),
        },
        Instruction::SparseSwitchData(r) => D::SparseSwitchPayload {
            keys_and_targets: r
                .keys
                .iter()
                .zip(r.targets.iter())
                .map(|(k, t)| (*k, *t))
                .collect(),
        },
        Instruction::FillArrayData(r) => D::FillArrayDataPayload {
            element_width: r.element_width,
            data: r.data.clone(),
        },
        Instruction::Raw(bytes) => {
            let code_units: smallvec::SmallVec<[u16; 5]> = bytes
                .chunks(2)
                .map(|c| {
                    if c.len() == 2 {
                        u16::from_le_bytes([c[0], c[1]])
                    } else {
                        u16::from(c[0])
                    }
                })
                .collect();
            D::RawInstruction { code_units }
        }
    }
}

fn intern_field(dex: &mut DexFile, field: &FieldRef) -> FieldIdx {
    dex.intern_field(&field.defining_class, &field.name, &field.field_type)
        .unwrap_or(FieldIdx(0))
}

fn intern_method(dex: &mut DexFile, method: &MethodRef) -> MethodIdx {
    dex.intern_method(&method.defining_class, &method.name, &method.proto)
        .unwrap_or(MethodIdx(0))
}

fn raw_from_opcode(opcode: u16) -> stitch_dex::Instruction {
    stitch_dex::Instruction::RawInstruction {
        code_units: smallvec::smallvec![opcode],
    }
}
