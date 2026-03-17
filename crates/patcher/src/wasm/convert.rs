//! Instruction conversion between stitch_dex::Instruction and WIT Instruction.
//!
//! Two functions convert losslessly (for supported opcodes) between the internal
//! ~230-variant Instruction enum and the 19-category WIT instruction variant.
//! Rare instructions (InvokeCustom, ConstMethodHandle, ConstMethodType) are
//! round-tripped through the `raw` variant.

use smallvec::SmallVec;
use stitch_apk::stitch_dex::{
    DexFile, FieldIdx, Instruction as DexInsn, MethodIdx, ProtoIdx,
};

pub use super::stitch::patch::types::{
    FieldRef as WitFieldRef, Instruction as WitInstruction, MethodRef as WitMethodRef,
};

// ── Helpers ──

fn resolve_method_ref(dex: &DexFile, method_idx: MethodIdx) -> WitMethodRef {
    let method_id = &dex.methods[method_idx.0 as usize];
    let class = dex.type_descriptor(method_id.class).to_string();
    let name = dex.string(method_id.name).to_string();
    let proto = proto_descriptor(dex, method_id.proto);
    WitMethodRef {
        defining_class: class,
        name,
        proto,
    }
}

fn resolve_field_ref(dex: &DexFile, field_idx: FieldIdx) -> WitFieldRef {
    let field_id = &dex.fields[field_idx.0 as usize];
    let class = dex.type_descriptor(field_id.class).to_string();
    let name = dex.string(field_id.name).to_string();
    let field_type = dex.type_descriptor(field_id.type_).to_string();
    WitFieldRef {
        defining_class: class,
        name,
        field_type,
    }
}

fn proto_descriptor(dex: &DexFile, proto_idx: ProtoIdx) -> String {
    let proto = &dex.prototypes[proto_idx.0 as usize];
    let ret = dex.type_descriptor(proto.return_type);
    let params: Vec<&str> = proto
        .parameters
        .iter()
        .map(|p| dex.type_descriptor(*p))
        .collect();
    format!("({}){}", params.join(""), ret)
}

fn args_to_u16(args: &SmallVec<[u8; 5]>) -> Vec<u16> {
    args.iter().map(|&a| a as u16).collect()
}

/// Encode an instruction as raw u16 code units → little-endian byte vec.
fn encode_raw(code_units: &[u16]) -> Vec<u8> {
    code_units.iter().flat_map(|u| u.to_le_bytes()).collect()
}

// ── dex_to_wit ──

pub fn dex_to_wit(insn: &DexInsn, dex: &DexFile) -> WitInstruction {
    use DexInsn as I;
    match insn {
        // simple
        I::Nop => WitInstruction::Simple(0x00),
        I::ReturnVoid => WitInstruction::Simple(0x0e),

        // reg1
        I::MoveResult { dest } => WitInstruction::Reg1((0x0a, *dest as u16)),
        I::MoveResultWide { dest } => WitInstruction::Reg1((0x0b, *dest as u16)),
        I::MoveResultObject { dest } => WitInstruction::Reg1((0x0c, *dest as u16)),
        I::MoveException { dest } => WitInstruction::Reg1((0x0d, *dest as u16)),
        I::Return { src } => WitInstruction::Reg1((0x0f, *src as u16)),
        I::ReturnWide { src } => WitInstruction::Reg1((0x10, *src as u16)),
        I::ReturnObject { src } => WitInstruction::Reg1((0x11, *src as u16)),
        I::MonitorEnter { ref_ } => WitInstruction::Reg1((0x1d, *ref_ as u16)),
        I::MonitorExit { ref_ } => WitInstruction::Reg1((0x1e, *ref_ as u16)),
        I::Throw { exception } => WitInstruction::Reg1((0x27, *exception as u16)),

        // reg2 — moves
        I::Move { dest, src } => WitInstruction::Reg2((0x01, *dest as u16, *src as u16)),
        I::MoveFrom16 { dest, src } => WitInstruction::Reg2((0x02, *dest as u16, *src)),
        I::Move16 { dest, src } => WitInstruction::Reg2((0x03, *dest, *src)),
        I::MoveWide { dest, src } => WitInstruction::Reg2((0x04, *dest as u16, *src as u16)),
        I::MoveWideFrom16 { dest, src } => WitInstruction::Reg2((0x05, *dest as u16, *src)),
        I::MoveWide16 { dest, src } => WitInstruction::Reg2((0x06, *dest, *src)),
        I::MoveObject { dest, src } => WitInstruction::Reg2((0x07, *dest as u16, *src as u16)),
        I::MoveObjectFrom16 { dest, src } => WitInstruction::Reg2((0x08, *dest as u16, *src)),
        I::MoveObject16 { dest, src } => WitInstruction::Reg2((0x09, *dest, *src)),
        I::ArrayLength { dest, array } => {
            WitInstruction::Reg2((0x21, *dest as u16, *array as u16))
        }

        // reg2 — unary/conversion ops
        I::NegInt { dest, src } => WitInstruction::Reg2((0x7b, *dest as u16, *src as u16)),
        I::NotInt { dest, src } => WitInstruction::Reg2((0x7c, *dest as u16, *src as u16)),
        I::NegLong { dest, src } => WitInstruction::Reg2((0x7d, *dest as u16, *src as u16)),
        I::NotLong { dest, src } => WitInstruction::Reg2((0x7e, *dest as u16, *src as u16)),
        I::NegFloat { dest, src } => WitInstruction::Reg2((0x7f, *dest as u16, *src as u16)),
        I::NegDouble { dest, src } => WitInstruction::Reg2((0x80, *dest as u16, *src as u16)),
        I::IntToLong { dest, src } => WitInstruction::Reg2((0x81, *dest as u16, *src as u16)),
        I::IntToFloat { dest, src } => WitInstruction::Reg2((0x82, *dest as u16, *src as u16)),
        I::IntToDouble { dest, src } => WitInstruction::Reg2((0x83, *dest as u16, *src as u16)),
        I::LongToInt { dest, src } => WitInstruction::Reg2((0x84, *dest as u16, *src as u16)),
        I::LongToFloat { dest, src } => WitInstruction::Reg2((0x85, *dest as u16, *src as u16)),
        I::LongToDouble { dest, src } => WitInstruction::Reg2((0x86, *dest as u16, *src as u16)),
        I::FloatToInt { dest, src } => WitInstruction::Reg2((0x87, *dest as u16, *src as u16)),
        I::FloatToLong { dest, src } => WitInstruction::Reg2((0x88, *dest as u16, *src as u16)),
        I::FloatToDouble { dest, src } => WitInstruction::Reg2((0x89, *dest as u16, *src as u16)),
        I::DoubleToInt { dest, src } => WitInstruction::Reg2((0x8a, *dest as u16, *src as u16)),
        I::DoubleToLong { dest, src } => WitInstruction::Reg2((0x8b, *dest as u16, *src as u16)),
        I::DoubleToFloat { dest, src } => WitInstruction::Reg2((0x8c, *dest as u16, *src as u16)),
        I::IntToByte { dest, src } => WitInstruction::Reg2((0x8d, *dest as u16, *src as u16)),
        I::IntToChar { dest, src } => WitInstruction::Reg2((0x8e, *dest as u16, *src as u16)),
        I::IntToShort { dest, src } => WitInstruction::Reg2((0x8f, *dest as u16, *src as u16)),

        // reg2 — 2addr ops
        I::AddInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb0, *dest_a as u16, *b as u16))
        }
        I::SubInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb1, *dest_a as u16, *b as u16))
        }
        I::MulInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb2, *dest_a as u16, *b as u16))
        }
        I::DivInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb3, *dest_a as u16, *b as u16))
        }
        I::RemInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb4, *dest_a as u16, *b as u16))
        }
        I::AndInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb5, *dest_a as u16, *b as u16))
        }
        I::OrInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb6, *dest_a as u16, *b as u16))
        }
        I::XorInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb7, *dest_a as u16, *b as u16))
        }
        I::ShlInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb8, *dest_a as u16, *b as u16))
        }
        I::ShrInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xb9, *dest_a as u16, *b as u16))
        }
        I::UshrInt2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xba, *dest_a as u16, *b as u16))
        }
        I::AddLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xbb, *dest_a as u16, *b as u16))
        }
        I::SubLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xbc, *dest_a as u16, *b as u16))
        }
        I::MulLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xbd, *dest_a as u16, *b as u16))
        }
        I::DivLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xbe, *dest_a as u16, *b as u16))
        }
        I::RemLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xbf, *dest_a as u16, *b as u16))
        }
        I::AndLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc0, *dest_a as u16, *b as u16))
        }
        I::OrLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc1, *dest_a as u16, *b as u16))
        }
        I::XorLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc2, *dest_a as u16, *b as u16))
        }
        I::ShlLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc3, *dest_a as u16, *b as u16))
        }
        I::ShrLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc4, *dest_a as u16, *b as u16))
        }
        I::UshrLong2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc5, *dest_a as u16, *b as u16))
        }
        I::AddFloat2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc6, *dest_a as u16, *b as u16))
        }
        I::SubFloat2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc7, *dest_a as u16, *b as u16))
        }
        I::MulFloat2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc8, *dest_a as u16, *b as u16))
        }
        I::DivFloat2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xc9, *dest_a as u16, *b as u16))
        }
        I::RemFloat2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xca, *dest_a as u16, *b as u16))
        }
        I::AddDouble2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xcb, *dest_a as u16, *b as u16))
        }
        I::SubDouble2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xcc, *dest_a as u16, *b as u16))
        }
        I::MulDouble2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xcd, *dest_a as u16, *b as u16))
        }
        I::DivDouble2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xce, *dest_a as u16, *b as u16))
        }
        I::RemDouble2Addr { dest_a, b } => {
            WitInstruction::Reg2((0xcf, *dest_a as u16, *b as u16))
        }

        // reg3 — aget
        I::Aget { dest, array, index } => {
            WitInstruction::Reg3((0x44, *dest as u16, *array as u16, *index as u16))
        }
        I::AgetWide { dest, array, index } => {
            WitInstruction::Reg3((0x45, *dest as u16, *array as u16, *index as u16))
        }
        I::AgetObject { dest, array, index } => {
            WitInstruction::Reg3((0x46, *dest as u16, *array as u16, *index as u16))
        }
        I::AgetBoolean { dest, array, index } => {
            WitInstruction::Reg3((0x47, *dest as u16, *array as u16, *index as u16))
        }
        I::AgetByte { dest, array, index } => {
            WitInstruction::Reg3((0x48, *dest as u16, *array as u16, *index as u16))
        }
        I::AgetChar { dest, array, index } => {
            WitInstruction::Reg3((0x49, *dest as u16, *array as u16, *index as u16))
        }
        I::AgetShort { dest, array, index } => {
            WitInstruction::Reg3((0x4a, *dest as u16, *array as u16, *index as u16))
        }

        // reg3 — aput
        I::Aput { src, array, index } => {
            WitInstruction::Reg3((0x4b, *src as u16, *array as u16, *index as u16))
        }
        I::AputWide { src, array, index } => {
            WitInstruction::Reg3((0x4c, *src as u16, *array as u16, *index as u16))
        }
        I::AputObject { src, array, index } => {
            WitInstruction::Reg3((0x4d, *src as u16, *array as u16, *index as u16))
        }
        I::AputBoolean { src, array, index } => {
            WitInstruction::Reg3((0x4e, *src as u16, *array as u16, *index as u16))
        }
        I::AputByte { src, array, index } => {
            WitInstruction::Reg3((0x4f, *src as u16, *array as u16, *index as u16))
        }
        I::AputChar { src, array, index } => {
            WitInstruction::Reg3((0x50, *src as u16, *array as u16, *index as u16))
        }
        I::AputShort { src, array, index } => {
            WitInstruction::Reg3((0x51, *src as u16, *array as u16, *index as u16))
        }

        // reg3 — cmp
        I::CmpLFloat { dest, a, b } => {
            WitInstruction::Reg3((0x2d, *dest as u16, *a as u16, *b as u16))
        }
        I::CmpGFloat { dest, a, b } => {
            WitInstruction::Reg3((0x2e, *dest as u16, *a as u16, *b as u16))
        }
        I::CmpLDouble { dest, a, b } => {
            WitInstruction::Reg3((0x2f, *dest as u16, *a as u16, *b as u16))
        }
        I::CmpGDouble { dest, a, b } => {
            WitInstruction::Reg3((0x30, *dest as u16, *a as u16, *b as u16))
        }
        I::CmpLong { dest, a, b } => {
            WitInstruction::Reg3((0x31, *dest as u16, *a as u16, *b as u16))
        }

        // reg3 — 3-register binary ops
        I::AddInt { dest, a, b } => {
            WitInstruction::Reg3((0x90, *dest as u16, *a as u16, *b as u16))
        }
        I::SubInt { dest, a, b } => {
            WitInstruction::Reg3((0x91, *dest as u16, *a as u16, *b as u16))
        }
        I::MulInt { dest, a, b } => {
            WitInstruction::Reg3((0x92, *dest as u16, *a as u16, *b as u16))
        }
        I::DivInt { dest, a, b } => {
            WitInstruction::Reg3((0x93, *dest as u16, *a as u16, *b as u16))
        }
        I::RemInt { dest, a, b } => {
            WitInstruction::Reg3((0x94, *dest as u16, *a as u16, *b as u16))
        }
        I::AndInt { dest, a, b } => {
            WitInstruction::Reg3((0x95, *dest as u16, *a as u16, *b as u16))
        }
        I::OrInt { dest, a, b } => {
            WitInstruction::Reg3((0x96, *dest as u16, *a as u16, *b as u16))
        }
        I::XorInt { dest, a, b } => {
            WitInstruction::Reg3((0x97, *dest as u16, *a as u16, *b as u16))
        }
        I::ShlInt { dest, a, b } => {
            WitInstruction::Reg3((0x98, *dest as u16, *a as u16, *b as u16))
        }
        I::ShrInt { dest, a, b } => {
            WitInstruction::Reg3((0x99, *dest as u16, *a as u16, *b as u16))
        }
        I::UshrInt { dest, a, b } => {
            WitInstruction::Reg3((0x9a, *dest as u16, *a as u16, *b as u16))
        }
        I::AddLong { dest, a, b } => {
            WitInstruction::Reg3((0x9b, *dest as u16, *a as u16, *b as u16))
        }
        I::SubLong { dest, a, b } => {
            WitInstruction::Reg3((0x9c, *dest as u16, *a as u16, *b as u16))
        }
        I::MulLong { dest, a, b } => {
            WitInstruction::Reg3((0x9d, *dest as u16, *a as u16, *b as u16))
        }
        I::DivLong { dest, a, b } => {
            WitInstruction::Reg3((0x9e, *dest as u16, *a as u16, *b as u16))
        }
        I::RemLong { dest, a, b } => {
            WitInstruction::Reg3((0x9f, *dest as u16, *a as u16, *b as u16))
        }
        I::AndLong { dest, a, b } => {
            WitInstruction::Reg3((0xa0, *dest as u16, *a as u16, *b as u16))
        }
        I::OrLong { dest, a, b } => {
            WitInstruction::Reg3((0xa1, *dest as u16, *a as u16, *b as u16))
        }
        I::XorLong { dest, a, b } => {
            WitInstruction::Reg3((0xa2, *dest as u16, *a as u16, *b as u16))
        }
        I::ShlLong { dest, a, b } => {
            WitInstruction::Reg3((0xa3, *dest as u16, *a as u16, *b as u16))
        }
        I::ShrLong { dest, a, b } => {
            WitInstruction::Reg3((0xa4, *dest as u16, *a as u16, *b as u16))
        }
        I::UshrLong { dest, a, b } => {
            WitInstruction::Reg3((0xa5, *dest as u16, *a as u16, *b as u16))
        }
        I::AddFloat { dest, a, b } => {
            WitInstruction::Reg3((0xa6, *dest as u16, *a as u16, *b as u16))
        }
        I::SubFloat { dest, a, b } => {
            WitInstruction::Reg3((0xa7, *dest as u16, *a as u16, *b as u16))
        }
        I::MulFloat { dest, a, b } => {
            WitInstruction::Reg3((0xa8, *dest as u16, *a as u16, *b as u16))
        }
        I::DivFloat { dest, a, b } => {
            WitInstruction::Reg3((0xa9, *dest as u16, *a as u16, *b as u16))
        }
        I::RemFloat { dest, a, b } => {
            WitInstruction::Reg3((0xaa, *dest as u16, *a as u16, *b as u16))
        }
        I::AddDouble { dest, a, b } => {
            WitInstruction::Reg3((0xab, *dest as u16, *a as u16, *b as u16))
        }
        I::SubDouble { dest, a, b } => {
            WitInstruction::Reg3((0xac, *dest as u16, *a as u16, *b as u16))
        }
        I::MulDouble { dest, a, b } => {
            WitInstruction::Reg3((0xad, *dest as u16, *a as u16, *b as u16))
        }
        I::DivDouble { dest, a, b } => {
            WitInstruction::Reg3((0xae, *dest as u16, *a as u16, *b as u16))
        }
        I::RemDouble { dest, a, b } => {
            WitInstruction::Reg3((0xaf, *dest as u16, *a as u16, *b as u16))
        }

        // reg-literal — const ops (src=0)
        I::Const4 { dest, value } => {
            WitInstruction::RegLiteral((0x12, *dest as u16, 0, *value as i64))
        }
        I::Const16 { dest, value } => {
            WitInstruction::RegLiteral((0x13, *dest as u16, 0, *value as i64))
        }
        I::Const { dest, value } => {
            WitInstruction::RegLiteral((0x14, *dest as u16, 0, *value as i64))
        }
        I::ConstHigh16 { dest, value } => {
            WitInstruction::RegLiteral((0x15, *dest as u16, 0, *value as i64))
        }
        I::ConstWide16 { dest, value } => {
            WitInstruction::RegLiteral((0x16, *dest as u16, 0, *value as i64))
        }
        I::ConstWide32 { dest, value } => {
            WitInstruction::RegLiteral((0x17, *dest as u16, 0, *value as i64))
        }
        I::ConstWide { dest, value } => {
            WitInstruction::RegLiteral((0x18, *dest as u16, 0, *value))
        }
        I::ConstWideHigh16 { dest, value } => {
            WitInstruction::RegLiteral((0x19, *dest as u16, 0, *value as i64))
        }

        // reg-literal — lit16 ops
        I::AddIntLit16 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd0, *dest as u16, *src as u16, *literal as i64))
        }
        I::RsubIntLit16 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd1, *dest as u16, *src as u16, *literal as i64))
        }
        I::MulIntLit16 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd2, *dest as u16, *src as u16, *literal as i64))
        }
        I::DivIntLit16 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd3, *dest as u16, *src as u16, *literal as i64))
        }
        I::RemIntLit16 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd4, *dest as u16, *src as u16, *literal as i64))
        }
        I::AndIntLit16 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd5, *dest as u16, *src as u16, *literal as i64))
        }
        I::OrIntLit16 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd6, *dest as u16, *src as u16, *literal as i64))
        }
        I::XorIntLit16 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd7, *dest as u16, *src as u16, *literal as i64))
        }

        // reg-literal — lit8 ops
        I::AddIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd8, *dest as u16, *src as u16, *literal as i64))
        }
        I::RsubIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xd9, *dest as u16, *src as u16, *literal as i64))
        }
        I::MulIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xda, *dest as u16, *src as u16, *literal as i64))
        }
        I::DivIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xdb, *dest as u16, *src as u16, *literal as i64))
        }
        I::RemIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xdc, *dest as u16, *src as u16, *literal as i64))
        }
        I::AndIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xdd, *dest as u16, *src as u16, *literal as i64))
        }
        I::OrIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xde, *dest as u16, *src as u16, *literal as i64))
        }
        I::XorIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xdf, *dest as u16, *src as u16, *literal as i64))
        }
        I::ShlIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xe0, *dest as u16, *src as u16, *literal as i64))
        }
        I::ShrIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xe1, *dest as u16, *src as u16, *literal as i64))
        }
        I::UshrIntLit8 { dest, src, literal } => {
            WitInstruction::RegLiteral((0xe2, *dest as u16, *src as u16, *literal as i64))
        }

        // reg-string
        I::ConstString { dest, string } => {
            WitInstruction::RegString((0x1a, *dest as u16, dex.string(*string).to_string()))
        }
        I::ConstStringJumbo { dest, string } => {
            WitInstruction::RegString((0x1b, *dest as u16, dex.string(*string).to_string()))
        }

        // reg-type
        I::ConstClass { dest, type_ } => {
            WitInstruction::RegType((0x1c, *dest as u16, 0, dex.type_descriptor(*type_).to_string()))
        }
        I::CheckCast { ref_, type_ } => {
            WitInstruction::RegType((0x1f, *ref_ as u16, 0, dex.type_descriptor(*type_).to_string()))
        }
        I::InstanceOf { dest, ref_, type_ } => WitInstruction::RegType((
            0x20,
            *dest as u16,
            *ref_ as u16,
            dex.type_descriptor(*type_).to_string(),
        )),
        I::NewInstance { dest, type_ } => {
            WitInstruction::RegType((0x22, *dest as u16, 0, dex.type_descriptor(*type_).to_string()))
        }
        I::NewArray { dest, size, type_ } => WitInstruction::RegType((
            0x23,
            *dest as u16,
            *size as u16,
            dex.type_descriptor(*type_).to_string(),
        )),

        // reg-field — iget
        I::Iget { dest, obj, field } => WitInstruction::RegField((
            0x52,
            *dest as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IgetWide { dest, obj, field } => WitInstruction::RegField((
            0x53,
            *dest as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IgetObject { dest, obj, field } => WitInstruction::RegField((
            0x54,
            *dest as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IgetBoolean { dest, obj, field } => WitInstruction::RegField((
            0x55,
            *dest as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IgetByte { dest, obj, field } => WitInstruction::RegField((
            0x56,
            *dest as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IgetChar { dest, obj, field } => WitInstruction::RegField((
            0x57,
            *dest as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IgetShort { dest, obj, field } => WitInstruction::RegField((
            0x58,
            *dest as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),

        // reg-field — iput
        I::Iput { src, obj, field } => WitInstruction::RegField((
            0x59,
            *src as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IputWide { src, obj, field } => WitInstruction::RegField((
            0x5a,
            *src as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IputObject { src, obj, field } => WitInstruction::RegField((
            0x5b,
            *src as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IputBoolean { src, obj, field } => WitInstruction::RegField((
            0x5c,
            *src as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IputByte { src, obj, field } => WitInstruction::RegField((
            0x5d,
            *src as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IputChar { src, obj, field } => WitInstruction::RegField((
            0x5e,
            *src as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),
        I::IputShort { src, obj, field } => WitInstruction::RegField((
            0x5f,
            *src as u16,
            *obj as u16,
            resolve_field_ref(dex, *field),
        )),

        // reg-field — sget (r2=0, no obj register)
        I::Sget { dest, field } => {
            WitInstruction::RegField((0x60, *dest as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SgetWide { dest, field } => {
            WitInstruction::RegField((0x61, *dest as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SgetObject { dest, field } => {
            WitInstruction::RegField((0x62, *dest as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SgetBoolean { dest, field } => {
            WitInstruction::RegField((0x63, *dest as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SgetByte { dest, field } => {
            WitInstruction::RegField((0x64, *dest as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SgetChar { dest, field } => {
            WitInstruction::RegField((0x65, *dest as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SgetShort { dest, field } => {
            WitInstruction::RegField((0x66, *dest as u16, 0, resolve_field_ref(dex, *field)))
        }

        // reg-field — sput (r2=0, no obj register)
        I::Sput { src, field } => {
            WitInstruction::RegField((0x68, *src as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SputWide { src, field } => {
            WitInstruction::RegField((0x69, *src as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SputObject { src, field } => {
            WitInstruction::RegField((0x6a, *src as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SputBoolean { src, field } => {
            WitInstruction::RegField((0x6b, *src as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SputByte { src, field } => {
            WitInstruction::RegField((0x6c, *src as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SputChar { src, field } => {
            WitInstruction::RegField((0x6d, *src as u16, 0, resolve_field_ref(dex, *field)))
        }
        I::SputShort { src, field } => {
            WitInstruction::RegField((0x6e, *src as u16, 0, resolve_field_ref(dex, *field)))
        }

        // invoke
        I::InvokeVirtual { method, args } => {
            WitInstruction::Invoke((0x6e, args_to_u16(args), resolve_method_ref(dex, *method)))
        }
        I::InvokeSuper { method, args } => {
            WitInstruction::Invoke((0x6f, args_to_u16(args), resolve_method_ref(dex, *method)))
        }
        I::InvokeDirect { method, args } => {
            WitInstruction::Invoke((0x70, args_to_u16(args), resolve_method_ref(dex, *method)))
        }
        I::InvokeStatic { method, args } => {
            WitInstruction::Invoke((0x71, args_to_u16(args), resolve_method_ref(dex, *method)))
        }
        I::InvokeInterface { method, args } => {
            WitInstruction::Invoke((0x72, args_to_u16(args), resolve_method_ref(dex, *method)))
        }
        // InvokePolymorphic — use call-site proto in MethodRef
        I::InvokePolymorphic {
            method,
            proto,
            args,
        } => {
            let mut mr = resolve_method_ref(dex, *method);
            mr.proto = proto_descriptor(dex, *proto);
            WitInstruction::Invoke((0xfa, args_to_u16(args), mr))
        }

        // invoke-range
        I::InvokeVirtualRange {
            method,
            first_reg,
            count,
        } => WitInstruction::InvokeRange((
            0x74,
            *first_reg,
            *count as u16,
            resolve_method_ref(dex, *method),
        )),
        I::InvokeSuperRange {
            method,
            first_reg,
            count,
        } => WitInstruction::InvokeRange((
            0x75,
            *first_reg,
            *count as u16,
            resolve_method_ref(dex, *method),
        )),
        I::InvokeDirectRange {
            method,
            first_reg,
            count,
        } => WitInstruction::InvokeRange((
            0x76,
            *first_reg,
            *count as u16,
            resolve_method_ref(dex, *method),
        )),
        I::InvokeStaticRange {
            method,
            first_reg,
            count,
        } => WitInstruction::InvokeRange((
            0x77,
            *first_reg,
            *count as u16,
            resolve_method_ref(dex, *method),
        )),
        I::InvokeInterfaceRange {
            method,
            first_reg,
            count,
        } => WitInstruction::InvokeRange((
            0x78,
            *first_reg,
            *count as u16,
            resolve_method_ref(dex, *method),
        )),
        I::InvokePolymorphicRange {
            method,
            proto,
            first_reg,
            count,
        } => {
            let mut mr = resolve_method_ref(dex, *method);
            mr.proto = proto_descriptor(dex, *proto);
            WitInstruction::InvokeRange((0xfb, *first_reg, *count as u16, mr))
        }

        // InvokeCustom/InvokeCustomRange → raw (no WIT representation for CallSiteIdx)
        I::InvokeCustom { call_site, args } => {
            let mut units: Vec<u16> = vec![0xfc | ((args.len() as u16) << 8)];
            units.push(call_site.0 as u16);
            // Encode args in 35c format
            let mut reg_unit: u16 = 0;
            for (i, &a) in args.iter().enumerate().take(4) {
                reg_unit |= (a as u16) << (i * 4);
            }
            units.push(reg_unit);
            if args.len() == 5 {
                units[0] |= (args[4] as u16) << 8; // A is in high nibble of first unit
            }
            WitInstruction::Raw(encode_raw(&units))
        }
        I::InvokeCustomRange {
            call_site,
            first_reg,
            count,
        } => {
            let units: Vec<u16> = vec![
                0xfd | ((*count as u16) << 8),
                call_site.0 as u16,
                *first_reg,
            ];
            WitInstruction::Raw(encode_raw(&units))
        }
        I::ConstMethodHandle {
            dest,
            method_handle,
        } => {
            let units: Vec<u16> = vec![0xfe | ((*dest as u16) << 8), method_handle.0 as u16];
            WitInstruction::Raw(encode_raw(&units))
        }
        I::ConstMethodType { dest, proto } => {
            let units: Vec<u16> = vec![0xff | ((*dest as u16) << 8), proto.0];
            WitInstruction::Raw(encode_raw(&units))
        }

        // branch0 — unconditional goto
        I::Goto { offset } => WitInstruction::Branch0((0x28, *offset as i32)),
        I::Goto16 { offset } => WitInstruction::Branch0((0x29, *offset as i32)),
        I::Goto32 { offset } => WitInstruction::Branch0((0x2a, *offset)),

        // branch — 1-register conditional + switch/fill
        I::IfEqz { a, offset } => WitInstruction::Branch((0x38, *a as u16, *offset as i32)),
        I::IfNez { a, offset } => WitInstruction::Branch((0x39, *a as u16, *offset as i32)),
        I::IfLtz { a, offset } => WitInstruction::Branch((0x3a, *a as u16, *offset as i32)),
        I::IfGez { a, offset } => WitInstruction::Branch((0x3b, *a as u16, *offset as i32)),
        I::IfGtz { a, offset } => WitInstruction::Branch((0x3c, *a as u16, *offset as i32)),
        I::IfLez { a, offset } => WitInstruction::Branch((0x3d, *a as u16, *offset as i32)),
        I::PackedSwitch {
            test,
            payload_offset,
        } => WitInstruction::Branch((0x2b, *test as u16, *payload_offset)),
        I::SparseSwitch {
            test,
            payload_offset,
        } => WitInstruction::Branch((0x2c, *test as u16, *payload_offset)),
        I::FillArrayData {
            array,
            payload_offset,
        } => WitInstruction::Branch((0x26, *array as u16, *payload_offset)),

        // branch2 — 2-register conditional
        I::IfEq { a, b, offset } => {
            WitInstruction::Branch2((0x32, *a as u16, *b as u16, *offset as i32))
        }
        I::IfNe { a, b, offset } => {
            WitInstruction::Branch2((0x33, *a as u16, *b as u16, *offset as i32))
        }
        I::IfLt { a, b, offset } => {
            WitInstruction::Branch2((0x34, *a as u16, *b as u16, *offset as i32))
        }
        I::IfGe { a, b, offset } => {
            WitInstruction::Branch2((0x35, *a as u16, *b as u16, *offset as i32))
        }
        I::IfGt { a, b, offset } => {
            WitInstruction::Branch2((0x36, *a as u16, *b as u16, *offset as i32))
        }
        I::IfLe { a, b, offset } => {
            WitInstruction::Branch2((0x37, *a as u16, *b as u16, *offset as i32))
        }

        // filled-array
        I::FilledNewArray { type_, args } => WitInstruction::FilledArray((
            0x24,
            args_to_u16(args),
            dex.type_descriptor(*type_).to_string(),
        )),

        // filled-array-range
        I::FilledNewArrayRange {
            type_,
            first_reg,
            count,
        } => WitInstruction::FilledArrayRange((
            0x25,
            *first_reg,
            *count as u16,
            dex.type_descriptor(*type_).to_string(),
        )),

        // payload data
        I::PackedSwitchPayload {
            first_key,
            targets,
        } => WitInstruction::PackedSwitchData((*first_key, targets.clone())),

        I::SparseSwitchPayload { keys_and_targets } => {
            let keys: Vec<i32> = keys_and_targets.iter().map(|&(k, _)| k).collect();
            let targets: Vec<i32> = keys_and_targets.iter().map(|&(_, t)| t).collect();
            WitInstruction::SparseSwitchData((keys, targets))
        }

        I::FillArrayDataPayload {
            element_width,
            data,
        } => WitInstruction::FillArrayData((*element_width, data.clone())),

        // raw
        I::RawInstruction { code_units } => WitInstruction::Raw(encode_raw(code_units)),
        _ => WitInstruction::Raw(Vec::new()),
    }
}

// ── wit_to_dex ──

fn intern_method(dex: &mut DexFile, mr: &WitMethodRef) -> Result<MethodIdx, String> {
    dex.intern_method(&mr.defining_class, &mr.name, &mr.proto)
        .map_err(|e| e.to_string())
}

fn intern_field(dex: &mut DexFile, fr: &WitFieldRef) -> Result<FieldIdx, String> {
    dex.intern_field(&fr.defining_class, &fr.name, &fr.field_type)
        .map_err(|e| e.to_string())
}

pub fn wit_to_dex(insn: &WitInstruction, dex: &mut DexFile) -> Result<DexInsn, String> {
    use WitInstruction as W;
    match insn {
        W::Simple(op) => match op {
            0x00 => Ok(DexInsn::Nop),
            0x0e => Ok(DexInsn::ReturnVoid),
            _ => Err(format!("unknown simple opcode: {:#x}", op)),
        },

        W::Reg1((op, r)) => match op {
            0x0a => Ok(DexInsn::MoveResult { dest: *r as u8 }),
            0x0b => Ok(DexInsn::MoveResultWide { dest: *r as u8 }),
            0x0c => Ok(DexInsn::MoveResultObject { dest: *r as u8 }),
            0x0d => Ok(DexInsn::MoveException { dest: *r as u8 }),
            0x0f => Ok(DexInsn::Return { src: *r as u8 }),
            0x10 => Ok(DexInsn::ReturnWide { src: *r as u8 }),
            0x11 => Ok(DexInsn::ReturnObject { src: *r as u8 }),
            0x1d => Ok(DexInsn::MonitorEnter { ref_: *r as u8 }),
            0x1e => Ok(DexInsn::MonitorExit { ref_: *r as u8 }),
            0x27 => Ok(DexInsn::Throw { exception: *r as u8 }),
            _ => Err(format!("unknown reg1 opcode: {:#x}", op)),
        },

        W::Reg2((op, r1, r2)) => match op {
            0x01 => Ok(DexInsn::Move { dest: *r1 as u8, src: *r2 as u8 }),
            0x02 => Ok(DexInsn::MoveFrom16 { dest: *r1 as u8, src: *r2 }),
            0x03 => Ok(DexInsn::Move16 { dest: *r1, src: *r2 }),
            0x04 => Ok(DexInsn::MoveWide { dest: *r1 as u8, src: *r2 as u8 }),
            0x05 => Ok(DexInsn::MoveWideFrom16 { dest: *r1 as u8, src: *r2 }),
            0x06 => Ok(DexInsn::MoveWide16 { dest: *r1, src: *r2 }),
            0x07 => Ok(DexInsn::MoveObject { dest: *r1 as u8, src: *r2 as u8 }),
            0x08 => Ok(DexInsn::MoveObjectFrom16 { dest: *r1 as u8, src: *r2 }),
            0x09 => Ok(DexInsn::MoveObject16 { dest: *r1, src: *r2 }),
            0x21 => Ok(DexInsn::ArrayLength { dest: *r1 as u8, array: *r2 as u8 }),
            // unary/conversion ops
            0x7b => Ok(DexInsn::NegInt { dest: *r1 as u8, src: *r2 as u8 }),
            0x7c => Ok(DexInsn::NotInt { dest: *r1 as u8, src: *r2 as u8 }),
            0x7d => Ok(DexInsn::NegLong { dest: *r1 as u8, src: *r2 as u8 }),
            0x7e => Ok(DexInsn::NotLong { dest: *r1 as u8, src: *r2 as u8 }),
            0x7f => Ok(DexInsn::NegFloat { dest: *r1 as u8, src: *r2 as u8 }),
            0x80 => Ok(DexInsn::NegDouble { dest: *r1 as u8, src: *r2 as u8 }),
            0x81 => Ok(DexInsn::IntToLong { dest: *r1 as u8, src: *r2 as u8 }),
            0x82 => Ok(DexInsn::IntToFloat { dest: *r1 as u8, src: *r2 as u8 }),
            0x83 => Ok(DexInsn::IntToDouble { dest: *r1 as u8, src: *r2 as u8 }),
            0x84 => Ok(DexInsn::LongToInt { dest: *r1 as u8, src: *r2 as u8 }),
            0x85 => Ok(DexInsn::LongToFloat { dest: *r1 as u8, src: *r2 as u8 }),
            0x86 => Ok(DexInsn::LongToDouble { dest: *r1 as u8, src: *r2 as u8 }),
            0x87 => Ok(DexInsn::FloatToInt { dest: *r1 as u8, src: *r2 as u8 }),
            0x88 => Ok(DexInsn::FloatToLong { dest: *r1 as u8, src: *r2 as u8 }),
            0x89 => Ok(DexInsn::FloatToDouble { dest: *r1 as u8, src: *r2 as u8 }),
            0x8a => Ok(DexInsn::DoubleToInt { dest: *r1 as u8, src: *r2 as u8 }),
            0x8b => Ok(DexInsn::DoubleToLong { dest: *r1 as u8, src: *r2 as u8 }),
            0x8c => Ok(DexInsn::DoubleToFloat { dest: *r1 as u8, src: *r2 as u8 }),
            0x8d => Ok(DexInsn::IntToByte { dest: *r1 as u8, src: *r2 as u8 }),
            0x8e => Ok(DexInsn::IntToChar { dest: *r1 as u8, src: *r2 as u8 }),
            0x8f => Ok(DexInsn::IntToShort { dest: *r1 as u8, src: *r2 as u8 }),
            // 2addr ops
            0xb0 => Ok(DexInsn::AddInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xb1 => Ok(DexInsn::SubInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xb2 => Ok(DexInsn::MulInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xb3 => Ok(DexInsn::DivInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xb4 => Ok(DexInsn::RemInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xb5 => Ok(DexInsn::AndInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xb6 => Ok(DexInsn::OrInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xb7 => Ok(DexInsn::XorInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xb8 => Ok(DexInsn::ShlInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xb9 => Ok(DexInsn::ShrInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xba => Ok(DexInsn::UshrInt2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xbb => Ok(DexInsn::AddLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xbc => Ok(DexInsn::SubLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xbd => Ok(DexInsn::MulLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xbe => Ok(DexInsn::DivLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xbf => Ok(DexInsn::RemLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc0 => Ok(DexInsn::AndLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc1 => Ok(DexInsn::OrLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc2 => Ok(DexInsn::XorLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc3 => Ok(DexInsn::ShlLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc4 => Ok(DexInsn::ShrLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc5 => Ok(DexInsn::UshrLong2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc6 => Ok(DexInsn::AddFloat2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc7 => Ok(DexInsn::SubFloat2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc8 => Ok(DexInsn::MulFloat2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xc9 => Ok(DexInsn::DivFloat2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xca => Ok(DexInsn::RemFloat2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xcb => Ok(DexInsn::AddDouble2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xcc => Ok(DexInsn::SubDouble2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xcd => Ok(DexInsn::MulDouble2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xce => Ok(DexInsn::DivDouble2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            0xcf => Ok(DexInsn::RemDouble2Addr { dest_a: *r1 as u8, b: *r2 as u8 }),
            _ => Err(format!("unknown reg2 opcode: {:#x}", op)),
        },

        W::Reg3((op, r1, r2, r3)) => match op {
            // aget
            0x44 => Ok(DexInsn::Aget { dest: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x45 => Ok(DexInsn::AgetWide { dest: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x46 => Ok(DexInsn::AgetObject { dest: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x47 => Ok(DexInsn::AgetBoolean { dest: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x48 => Ok(DexInsn::AgetByte { dest: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x49 => Ok(DexInsn::AgetChar { dest: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x4a => Ok(DexInsn::AgetShort { dest: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            // aput
            0x4b => Ok(DexInsn::Aput { src: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x4c => Ok(DexInsn::AputWide { src: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x4d => Ok(DexInsn::AputObject { src: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x4e => Ok(DexInsn::AputBoolean { src: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x4f => Ok(DexInsn::AputByte { src: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x50 => Ok(DexInsn::AputChar { src: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            0x51 => Ok(DexInsn::AputShort { src: *r1 as u8, array: *r2 as u8, index: *r3 as u8 }),
            // cmp
            0x2d => Ok(DexInsn::CmpLFloat { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x2e => Ok(DexInsn::CmpGFloat { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x2f => Ok(DexInsn::CmpLDouble { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x30 => Ok(DexInsn::CmpGDouble { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x31 => Ok(DexInsn::CmpLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            // 3-register binary
            0x90 => Ok(DexInsn::AddInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x91 => Ok(DexInsn::SubInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x92 => Ok(DexInsn::MulInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x93 => Ok(DexInsn::DivInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x94 => Ok(DexInsn::RemInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x95 => Ok(DexInsn::AndInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x96 => Ok(DexInsn::OrInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x97 => Ok(DexInsn::XorInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x98 => Ok(DexInsn::ShlInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x99 => Ok(DexInsn::ShrInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x9a => Ok(DexInsn::UshrInt { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x9b => Ok(DexInsn::AddLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x9c => Ok(DexInsn::SubLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x9d => Ok(DexInsn::MulLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x9e => Ok(DexInsn::DivLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0x9f => Ok(DexInsn::RemLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa0 => Ok(DexInsn::AndLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa1 => Ok(DexInsn::OrLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa2 => Ok(DexInsn::XorLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa3 => Ok(DexInsn::ShlLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa4 => Ok(DexInsn::ShrLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa5 => Ok(DexInsn::UshrLong { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa6 => Ok(DexInsn::AddFloat { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa7 => Ok(DexInsn::SubFloat { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa8 => Ok(DexInsn::MulFloat { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xa9 => Ok(DexInsn::DivFloat { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xaa => Ok(DexInsn::RemFloat { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xab => Ok(DexInsn::AddDouble { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xac => Ok(DexInsn::SubDouble { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xad => Ok(DexInsn::MulDouble { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xae => Ok(DexInsn::DivDouble { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            0xaf => Ok(DexInsn::RemDouble { dest: *r1 as u8, a: *r2 as u8, b: *r3 as u8 }),
            _ => Err(format!("unknown reg3 opcode: {:#x}", op)),
        },

        W::RegLiteral((op, dest, src, value)) => match op {
            // const ops (src is unused/0)
            0x12 => Ok(DexInsn::Const4 { dest: *dest as u8, value: *value as i8 }),
            0x13 => Ok(DexInsn::Const16 { dest: *dest as u8, value: *value as i16 }),
            0x14 => Ok(DexInsn::Const { dest: *dest as u8, value: *value as i32 }),
            0x15 => Ok(DexInsn::ConstHigh16 { dest: *dest as u8, value: *value as i16 }),
            0x16 => Ok(DexInsn::ConstWide16 { dest: *dest as u8, value: *value as i16 }),
            0x17 => Ok(DexInsn::ConstWide32 { dest: *dest as u8, value: *value as i32 }),
            0x18 => Ok(DexInsn::ConstWide { dest: *dest as u8, value: *value }),
            0x19 => Ok(DexInsn::ConstWideHigh16 { dest: *dest as u8, value: *value as i16 }),
            // lit16 ops
            0xd0 => Ok(DexInsn::AddIntLit16 { dest: *dest as u8, src: *src as u8, literal: *value as i16 }),
            0xd1 => Ok(DexInsn::RsubIntLit16 { dest: *dest as u8, src: *src as u8, literal: *value as i16 }),
            0xd2 => Ok(DexInsn::MulIntLit16 { dest: *dest as u8, src: *src as u8, literal: *value as i16 }),
            0xd3 => Ok(DexInsn::DivIntLit16 { dest: *dest as u8, src: *src as u8, literal: *value as i16 }),
            0xd4 => Ok(DexInsn::RemIntLit16 { dest: *dest as u8, src: *src as u8, literal: *value as i16 }),
            0xd5 => Ok(DexInsn::AndIntLit16 { dest: *dest as u8, src: *src as u8, literal: *value as i16 }),
            0xd6 => Ok(DexInsn::OrIntLit16 { dest: *dest as u8, src: *src as u8, literal: *value as i16 }),
            0xd7 => Ok(DexInsn::XorIntLit16 { dest: *dest as u8, src: *src as u8, literal: *value as i16 }),
            // lit8 ops
            0xd8 => Ok(DexInsn::AddIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xd9 => Ok(DexInsn::RsubIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xda => Ok(DexInsn::MulIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xdb => Ok(DexInsn::DivIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xdc => Ok(DexInsn::RemIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xdd => Ok(DexInsn::AndIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xde => Ok(DexInsn::OrIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xdf => Ok(DexInsn::XorIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xe0 => Ok(DexInsn::ShlIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xe1 => Ok(DexInsn::ShrIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            0xe2 => Ok(DexInsn::UshrIntLit8 { dest: *dest as u8, src: *src as u8, literal: *value as i8 }),
            _ => Err(format!("unknown reg-literal opcode: {:#x}", op)),
        },

        W::RegString((op, r, s)) => match op {
            0x1a => Ok(DexInsn::ConstString { dest: *r as u8, string: dex.intern_string(s) }),
            0x1b => Ok(DexInsn::ConstStringJumbo { dest: *r as u8, string: dex.intern_string(s) }),
            _ => Err(format!("unknown reg-string opcode: {:#x}", op)),
        },

        W::RegType((op, r1, r2, t)) => match op {
            0x1c => Ok(DexInsn::ConstClass { dest: *r1 as u8, type_: dex.intern_type(t) }),
            0x1f => Ok(DexInsn::CheckCast { ref_: *r1 as u8, type_: dex.intern_type(t) }),
            0x20 => Ok(DexInsn::InstanceOf { dest: *r1 as u8, ref_: *r2 as u8, type_: dex.intern_type(t) }),
            0x22 => Ok(DexInsn::NewInstance { dest: *r1 as u8, type_: dex.intern_type(t) }),
            0x23 => Ok(DexInsn::NewArray { dest: *r1 as u8, size: *r2 as u8, type_: dex.intern_type(t) }),
            _ => Err(format!("unknown reg-type opcode: {:#x}", op)),
        },

        W::RegField((op, r1, r2, fr)) => {
            let field = intern_field(dex, fr)?;
            match op {
                // iget
                0x52 => Ok(DexInsn::Iget { dest: *r1 as u8, obj: *r2 as u8, field }),
                0x53 => Ok(DexInsn::IgetWide { dest: *r1 as u8, obj: *r2 as u8, field }),
                0x54 => Ok(DexInsn::IgetObject { dest: *r1 as u8, obj: *r2 as u8, field }),
                0x55 => Ok(DexInsn::IgetBoolean { dest: *r1 as u8, obj: *r2 as u8, field }),
                0x56 => Ok(DexInsn::IgetByte { dest: *r1 as u8, obj: *r2 as u8, field }),
                0x57 => Ok(DexInsn::IgetChar { dest: *r1 as u8, obj: *r2 as u8, field }),
                0x58 => Ok(DexInsn::IgetShort { dest: *r1 as u8, obj: *r2 as u8, field }),
                // iput
                0x59 => Ok(DexInsn::Iput { src: *r1 as u8, obj: *r2 as u8, field }),
                0x5a => Ok(DexInsn::IputWide { src: *r1 as u8, obj: *r2 as u8, field }),
                0x5b => Ok(DexInsn::IputObject { src: *r1 as u8, obj: *r2 as u8, field }),
                0x5c => Ok(DexInsn::IputBoolean { src: *r1 as u8, obj: *r2 as u8, field }),
                0x5d => Ok(DexInsn::IputByte { src: *r1 as u8, obj: *r2 as u8, field }),
                0x5e => Ok(DexInsn::IputChar { src: *r1 as u8, obj: *r2 as u8, field }),
                0x5f => Ok(DexInsn::IputShort { src: *r1 as u8, obj: *r2 as u8, field }),
                // sget (r2 is unused/0)
                0x60 => Ok(DexInsn::Sget { dest: *r1 as u8, field }),
                0x61 => Ok(DexInsn::SgetWide { dest: *r1 as u8, field }),
                0x62 => Ok(DexInsn::SgetObject { dest: *r1 as u8, field }),
                0x63 => Ok(DexInsn::SgetBoolean { dest: *r1 as u8, field }),
                0x64 => Ok(DexInsn::SgetByte { dest: *r1 as u8, field }),
                0x65 => Ok(DexInsn::SgetChar { dest: *r1 as u8, field }),
                0x66 => Ok(DexInsn::SgetShort { dest: *r1 as u8, field }),
                // sput (r2 is unused/0)
                0x68 => Ok(DexInsn::Sput { src: *r1 as u8, field }),
                0x69 => Ok(DexInsn::SputWide { src: *r1 as u8, field }),
                0x6a => Ok(DexInsn::SputObject { src: *r1 as u8, field }),
                0x6b => Ok(DexInsn::SputBoolean { src: *r1 as u8, field }),
                0x6c => Ok(DexInsn::SputByte { src: *r1 as u8, field }),
                0x6d => Ok(DexInsn::SputChar { src: *r1 as u8, field }),
                0x6e => Ok(DexInsn::SputShort { src: *r1 as u8, field }),
                _ => Err(format!("unknown reg-field opcode: {:#x}", op)),
            }
        }

        W::Invoke((op, args, mr)) => {
            let u8_args: SmallVec<[u8; 5]> = args.iter().map(|&a| a as u8).collect();
            match op {
                0x6e => Ok(DexInsn::InvokeVirtual { method: intern_method(dex, mr)?, args: u8_args }),
                0x6f => Ok(DexInsn::InvokeSuper { method: intern_method(dex, mr)?, args: u8_args }),
                0x70 => Ok(DexInsn::InvokeDirect { method: intern_method(dex, mr)?, args: u8_args }),
                0x71 => Ok(DexInsn::InvokeStatic { method: intern_method(dex, mr)?, args: u8_args }),
                0x72 => Ok(DexInsn::InvokeInterface { method: intern_method(dex, mr)?, args: u8_args }),
                0xfa => {
                    // InvokePolymorphic: mr.proto is the call-site proto, not the method's own
                    let method_id = dex.intern_method(&mr.defining_class, &mr.name, &mr.proto)
                        .map_err(|e| e.to_string())?;
                    let proto = dex.intern_proto(&mr.proto).map_err(|e| e.to_string())?;
                    Ok(DexInsn::InvokePolymorphic { method: method_id, proto, args: u8_args })
                }
                _ => Err(format!("unknown invoke opcode: {:#x}", op)),
            }
        }

        W::InvokeRange((op, first_reg, count, mr)) => match op {
            0x74 => Ok(DexInsn::InvokeVirtualRange { method: intern_method(dex, mr)?, first_reg: *first_reg, count: *count as u8 }),
            0x75 => Ok(DexInsn::InvokeSuperRange { method: intern_method(dex, mr)?, first_reg: *first_reg, count: *count as u8 }),
            0x76 => Ok(DexInsn::InvokeDirectRange { method: intern_method(dex, mr)?, first_reg: *first_reg, count: *count as u8 }),
            0x77 => Ok(DexInsn::InvokeStaticRange { method: intern_method(dex, mr)?, first_reg: *first_reg, count: *count as u8 }),
            0x78 => Ok(DexInsn::InvokeInterfaceRange { method: intern_method(dex, mr)?, first_reg: *first_reg, count: *count as u8 }),
            0xfb => {
                let method_id = dex.intern_method(&mr.defining_class, &mr.name, &mr.proto)
                    .map_err(|e| e.to_string())?;
                let proto = dex.intern_proto(&mr.proto).map_err(|e| e.to_string())?;
                Ok(DexInsn::InvokePolymorphicRange { method: method_id, proto, first_reg: *first_reg, count: *count as u8 })
            }
            _ => Err(format!("unknown invoke-range opcode: {:#x}", op)),
        },

        W::Branch0((op, offset)) => match op {
            0x28 => Ok(DexInsn::Goto { offset: *offset as i8 }),
            0x29 => Ok(DexInsn::Goto16 { offset: *offset as i16 }),
            0x2a => Ok(DexInsn::Goto32 { offset: *offset }),
            _ => Err(format!("unknown branch0 opcode: {:#x}", op)),
        },

        W::Branch((op, r, offset)) => match op {
            0x26 => Ok(DexInsn::FillArrayData { array: *r as u8, payload_offset: *offset }),
            0x2b => Ok(DexInsn::PackedSwitch { test: *r as u8, payload_offset: *offset }),
            0x2c => Ok(DexInsn::SparseSwitch { test: *r as u8, payload_offset: *offset }),
            0x38 => Ok(DexInsn::IfEqz { a: *r as u8, offset: *offset as i16 }),
            0x39 => Ok(DexInsn::IfNez { a: *r as u8, offset: *offset as i16 }),
            0x3a => Ok(DexInsn::IfLtz { a: *r as u8, offset: *offset as i16 }),
            0x3b => Ok(DexInsn::IfGez { a: *r as u8, offset: *offset as i16 }),
            0x3c => Ok(DexInsn::IfGtz { a: *r as u8, offset: *offset as i16 }),
            0x3d => Ok(DexInsn::IfLez { a: *r as u8, offset: *offset as i16 }),
            _ => Err(format!("unknown branch opcode: {:#x}", op)),
        },

        W::Branch2((op, r1, r2, offset)) => match op {
            0x32 => Ok(DexInsn::IfEq { a: *r1 as u8, b: *r2 as u8, offset: *offset as i16 }),
            0x33 => Ok(DexInsn::IfNe { a: *r1 as u8, b: *r2 as u8, offset: *offset as i16 }),
            0x34 => Ok(DexInsn::IfLt { a: *r1 as u8, b: *r2 as u8, offset: *offset as i16 }),
            0x35 => Ok(DexInsn::IfGe { a: *r1 as u8, b: *r2 as u8, offset: *offset as i16 }),
            0x36 => Ok(DexInsn::IfGt { a: *r1 as u8, b: *r2 as u8, offset: *offset as i16 }),
            0x37 => Ok(DexInsn::IfLe { a: *r1 as u8, b: *r2 as u8, offset: *offset as i16 }),
            _ => Err(format!("unknown branch2 opcode: {:#x}", op)),
        },

        W::FilledArray((op, args, t)) => match op {
            0x24 => {
                let u8_args: SmallVec<[u8; 5]> = args.iter().map(|&a| a as u8).collect();
                Ok(DexInsn::FilledNewArray { type_: dex.intern_type(t), args: u8_args })
            }
            _ => Err(format!("unknown filled-array opcode: {:#x}", op)),
        },

        W::FilledArrayRange((op, first_reg, count, t)) => match op {
            0x25 => {
                Ok(DexInsn::FilledNewArrayRange { type_: dex.intern_type(t), first_reg: *first_reg, count: *count as u8 })
            }
            _ => Err(format!("unknown filled-array-range opcode: {:#x}", op)),
        },

        W::PackedSwitchData((first_key, targets)) => {
            Ok(DexInsn::PackedSwitchPayload { first_key: *first_key, targets: targets.clone() })
        }

        W::SparseSwitchData((keys, targets)) => {
            let keys_and_targets: Vec<(i32, i32)> = keys.iter().copied().zip(targets.iter().copied()).collect();
            Ok(DexInsn::SparseSwitchPayload { keys_and_targets })
        }

        W::FillArrayData((width, data)) => {
            Ok(DexInsn::FillArrayDataPayload { element_width: *width, data: data.clone() })
        }

        W::Raw(bytes) => {
            let code_units: SmallVec<[u16; 5]> = bytes
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            Ok(DexInsn::RawInstruction { code_units })
        }
    }
}
