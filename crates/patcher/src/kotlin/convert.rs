// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_apk::reseam_dex::{self, DexFile, FieldIdx, MethodIdx};

pub(super) fn resolve_method_ref(dex: &DexFile, idx: MethodIdx) -> MethodRef {
    let mid = dex.method_id(idx);
    let class = dex.type_descriptor(mid.class).into_owned();
    let name = dex.string(mid.name).into_owned();
    MethodRef {
        defining_class: class,
        name,
        proto: dex.proto_descriptor(&dex.proto(mid.proto)),
    }
}

pub(super) fn resolve_field_ref(dex: &DexFile, idx: FieldIdx) -> FieldRef {
    let fid = dex.field_id(idx);
    FieldRef {
        defining_class: dex.type_descriptor(fid.class).into_owned(),
        name: dex.string(fid.name).into_owned(),
        field_type: dex.type_descriptor(fid.type_).into_owned(),
    }
}

use super::pool_values::{export_call_site, export_handle, import_call_site, import_handle};
pub(super) use super::pool_values::{export_value, import_value};
use super::types::{
    Branch0Insn, Branch2Insn, BranchInsn, CustomInsn, CustomRangeInsn, FieldRef, FillArrayInsn,
    FilledArrayInsn, FilledArrayRangeInsn, Instruction, InvokeInsn, InvokeRangeInsn, MethodRef,
    PackedSwitchInsn, PolymorphicInsn, PolymorphicRangeInsn, Reg1Insn, Reg2Insn, Reg3Insn,
    RegFieldInsn, RegHandleInsn, RegLiteralInsn, RegProtoInsn, RegStringInsn, RegTypeInsn,
    SimpleInsn, SparseSwitchInsn,
};
use reseam_dex::{DexError, Instruction as D};

pub(super) fn invalid(reason: impl Into<String>) -> DexError {
    DexError::Invalid {
        section: "patch instruction",
        reason: reason.into(),
    }
}

macro_rules! export_field {
    (StringIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        $dex.string(*$value).into_owned()
    };
    (TypeIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        $dex.type_descriptor(*$value).into_owned()
    };
    (ProtoIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        $dex.proto_descriptor(&$dex.proto(*$value))
    };
    (MethodIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        resolve_method_ref($dex, *$value)
    };
    (FieldIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        resolve_field_ref($dex, *$value)
    };
    (MethodHandleIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        export_handle($dex, $dex_index, *$value)?
    };
    (CallSiteIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        export_call_site($dex, $dex_index, *$value)?
    };
    (RegList, $value:expr, $dex:ident, $dex_index:ident) => {
        $value.iter().map(|r| u16::from(*r)).collect()
    };
    ($numeric:ident, $value:expr, $dex:ident, $dex_index:ident) => {
        (*$value).into()
    };
}

macro_rules! import_field {
    (StringIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        Ok::<_, DexError>($dex.intern_string($value))
    };
    (TypeIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        $dex.intern_type($value)
    };
    (ProtoIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        $dex.intern_proto($value)
    };
    (MethodIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        $dex.intern_method(&$value.defining_class, &$value.name, &$value.proto)
    };
    (FieldIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        $dex.intern_field(&$value.defining_class, &$value.name, &$value.field_type)
    };
    (MethodHandleIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        import_handle($dex, $dex_index, $value)
    };
    (CallSiteIdx, $value:expr, $dex:ident, $dex_index:ident) => {
        import_call_site($dex, $dex_index, $value)
    };
    (RegList, $value:expr, $dex:ident, $dex_index:ident) => {
        reseam_dex::RegList::try_from_iter(
            $value
                .iter()
                .map(|r| u8::try_from(*r))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| invalid("compact register exceeds 255"))?,
        )
    };
    ($numeric:ident, $value:expr, $dex:ident, $dex_index:ident) => {
        $numeric::try_from(*$value)
            .map_err(|_| invalid(concat!("operand exceeds ", stringify!($numeric))))
    };
}

macro_rules! shape {
    ($apply:ident, $ctx:tt, []) => { $apply!($ctx, Simple, SimpleInsn, [], []) };
    ($apply:ident, $ctx:tt, [{method: MethodIdx, proto: ProtoIdx, args: RegList,}]) => { $apply!($ctx, Polymorphic, PolymorphicInsn, [method:MethodIdx=>method,proto:ProtoIdx=>proto,args:RegList=>registers], []) };
    ($apply:ident, $ctx:tt, [{method: MethodIdx, proto: ProtoIdx, first_reg: u16, count: u8,}]) => { $apply!($ctx, PolymorphicRange, PolymorphicRangeInsn, [method:MethodIdx=>method,proto:ProtoIdx=>proto,first_reg:u16=>start_reg,count:u8=>reg_count], []) };
    ($apply:ident, $ctx:tt, [{call_site: CallSiteIdx, args: RegList,}]) => { $apply!($ctx, Custom, CustomInsn, [call_site:CallSiteIdx=>call_site,args:RegList=>registers], []) };
    ($apply:ident, $ctx:tt, [{call_site: CallSiteIdx, first_reg: u16, count: u8,}]) => { $apply!($ctx, CustomRange, CustomRangeInsn, [call_site:CallSiteIdx=>call_site,first_reg:u16=>start_reg,count:u8=>reg_count], []) };
    ($apply:ident, $ctx:tt, [{dest: u8, method_handle: MethodHandleIdx,}]) => { $apply!($ctx, RegHandle, RegHandleInsn, [dest:u8=>reg_a,method_handle:MethodHandleIdx=>handle], []) };
    ($apply:ident, $ctx:tt, [{dest: u8, proto: ProtoIdx,}]) => { $apply!($ctx, RegProto, RegProtoInsn, [dest:u8=>reg_a,proto:ProtoIdx=>proto], []) };
    ($apply:ident, $ctx:tt, [{method: MethodIdx, args: RegList,}]) => { $apply!($ctx, Invoke, InvokeInsn, [method:MethodIdx=>method,args:RegList=>registers], []) };
    ($apply:ident, $ctx:tt, [{method: MethodIdx, first_reg: u16, count: u8,}]) => { $apply!($ctx, InvokeRange, InvokeRangeInsn, [method:MethodIdx=>method,first_reg:u16=>start_reg,count:u8=>reg_count], []) };
    ($apply:ident, $ctx:tt, [{type_: TypeIdx, args: RegList,}]) => { $apply!($ctx, FilledArray, FilledArrayInsn, [type_:TypeIdx=>type_descriptor,args:RegList=>registers], []) };
    ($apply:ident, $ctx:tt, [{type_: TypeIdx, first_reg: u16, count: u8,}]) => { $apply!($ctx, FilledArrayRange, FilledArrayRangeInsn, [type_:TypeIdx=>type_descriptor,first_reg:u16=>start_reg,count:u8=>reg_count], []) };
    ($apply:ident, $ctx:tt, [{offset: $t:ident,}]) => { $apply!($ctx, Branch0, Branch0Insn, [offset:$t=>offset], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, $b:ident:$tb:ident, offset:$t:ident,}]) => { $apply!($ctx, Branch2, Branch2Insn, [$a:$ta=>reg_a,$b:$tb=>reg_b,offset:$t=>offset], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, offset:$t:ident,}]) => { $apply!($ctx, Branch, BranchInsn, [$a:$ta=>reg_a,offset:$t=>offset], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, payload_offset:i32,}]) => { $apply!($ctx, Branch, BranchInsn, [$a:$ta=>reg_a,payload_offset:i32=>offset], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, value:$t:ident,}]) => { $apply!($ctx, RegLiteral, RegLiteralInsn, [$a:$ta=>reg_a,value:$t=>literal], [reg_b:0]) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, $b:ident:$tb:ident, literal:$t:ident,}]) => { $apply!($ctx, RegLiteral, RegLiteralInsn, [$a:$ta=>reg_a,$b:$tb=>reg_b,literal:$t=>literal], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, string:StringIdx,}]) => { $apply!($ctx, RegString, RegStringInsn, [$a:$ta=>reg_a,string:StringIdx=>value], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, $b:ident:$tb:ident, type_:TypeIdx,}]) => { $apply!($ctx, RegType, RegTypeInsn, [$a:$ta=>reg_a,$b:$tb=>reg_b,type_:TypeIdx=>type_descriptor], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, type_:TypeIdx,}]) => { $apply!($ctx, RegType, RegTypeInsn, [$a:$ta=>reg_a,type_:TypeIdx=>type_descriptor], [reg_b:0]) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, $b:ident:$tb:ident, field:FieldIdx,}]) => { $apply!($ctx, RegField, RegFieldInsn, [$a:$ta=>reg_a,$b:$tb=>reg_b,field:FieldIdx=>field], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, field:FieldIdx,}]) => { $apply!($ctx, RegField, RegFieldInsn, [$a:$ta=>reg_a,field:FieldIdx=>field], [reg_b:0]) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, $b:ident:$tb:ident, $c:ident:$tc:ident,}]) => { $apply!($ctx, Reg3, Reg3Insn, [$a:$ta=>reg_a,$b:$tb=>reg_b,$c:$tc=>reg_c], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident, $b:ident:$tb:ident,}]) => { $apply!($ctx, Reg2, Reg2Insn, [$a:$ta=>reg_a,$b:$tb=>reg_b], []) };
    ($apply:ident, $ctx:tt, [{$a:ident:$ta:ident,}]) => { $apply!($ctx, Reg1, Reg1Insn, [$a:$ta=>reg_a], []) };
    ($apply:ident, $ctx:tt, [$($special:tt)*]) => { $apply!($ctx, special) };
}

macro_rules! export_shape {
    (($insn:ident,$dex:ident,$dex_index:ident,$native:ident,$opcode:expr), $wire:ident, $record:ident, [$($field:ident:$ty:ident=>$slot:ident),*], [$($padding:ident:$zero:expr),*]) => {{
        let D::$native { $($field),* } = $insn else { return Err(invalid("catalogue does not match instruction operands")) };
        Instruction::$wire($record { opcode:$opcode.ok_or_else(|| invalid("structured instruction has no opcode"))?, $($slot:export_field!($ty,$field,$dex,$dex_index),)* $($padding:$zero,)* })
    }};
    (($insn:ident,$dex:ident,$dex_index:ident,$native:ident,$opcode:expr), special) => { export_payload($insn)? };
}

macro_rules! import_shape {
    (($insn:ident,$dex:ident,$dex_index:ident,$native:ident), $wire:ident, $record:ident, [$($field:ident:$ty:ident=>$slot:ident),*], [$($padding:ident:$zero:expr),*]) => {{
        let Instruction::$wire(_record) = $insn else { return Err(invalid("opcode does not match instruction operands")); };
        $(if _record.$padding != $zero { return Err(invalid("unused operand must be zero")); })*
        Ok::<D, DexError>(D::$native { $($field:import_field!($ty,&_record.$slot,$dex,$dex_index)?,)* })
    }};
    (($insn:ident,$dex:ident,$dex_index:ident,$native:ident), special) => { Err::<D, DexError>(invalid("payload requires its dedicated representation")) };
}

macro_rules! validate_arguments {
    (none, $instruction:ident, $native:ident) => {};
    (list, $instruction:ident, $native:ident) => {
        let D::$native { args, .. } = &$instruction else {
            return Err(invalid("opcode does not match list arguments"));
        };
        if args.iter().any(|register| *register > 15) {
            return Err(invalid("compact argument exceeds four bits"));
        }
    };
    (range, $instruction:ident, $native:ident) => {
        let D::$native {
            first_reg, count, ..
        } = &$instruction
        else {
            return Err(invalid("opcode does not match range arguments"));
        };
        if u32::from(*first_reg) + u32::from(*count) > 65536 {
            return Err(invalid("range arguments exceed the register address space"));
        }
    };
}

macro_rules! define_bridge {
    ($($native:ident [$($pattern:tt)*] [$($definition:tt)*] => $name:ident $opcode:expr,$units:tt; [$($reg:ident:$rt:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident:$it:ident $pool:ident $at:literal $width:ident),*];)*) => {
        pub fn dex_to_kotlin(insn: &D, dex: &DexFile, dex_index: Option<usize>) -> reseam_dex::Result<Instruction> {
            Ok(match insn {
                $(D::$native $($pattern)* => shape!(export_shape,(insn,dex,dex_index,$native,$opcode),[$($definition)*]),)*
                _ => export_payload(insn)?,
            })
        }
        fn structured(insn: &Instruction, opcode:u16, dex:&mut DexFile, dex_index:Option<usize>) -> reseam_dex::Result<D> {
            match opcode {
                $(value if Some(value) == $opcode => {
                    let instruction = shape!(import_shape,(insn,dex,dex_index,$native),[$($definition)*])?;
                    let D::$native { $($reg,)* .. } = &instruction else { return Err(invalid("opcode does not match instruction operands")) };
                    $(if u16::from(*$reg) > $max { return Err(invalid("register exceeds instruction encoding")); })*
                    if let D::Const4 { value, .. } = instruction && !(-8..=7).contains(&value) {
                        return Err(invalid("const/4 literal exceeds four bits"));
                    }
                    validate_arguments!($args, instruction, $native);
                    Ok(instruction)
                },)*
                _ => Err(invalid(format!("unsupported structured opcode {opcode:#x}"))),
            }
        }
    };
}
reseam_dex::instruction_catalogue!(define_bridge);
fn export_payload(instruction: &D) -> reseam_dex::Result<Instruction> {
    Ok(match instruction {
        D::PackedSwitchPayload(value) => Instruction::PackedSwitchData(PackedSwitchInsn {
            first_key: value.first_key,
            targets: value.targets.clone(),
        }),
        D::SparseSwitchPayload(value) => Instruction::SparseSwitchData(SparseSwitchInsn {
            keys: value.keys_and_targets.iter().map(|(key, _)| *key).collect(),
            targets: value
                .keys_and_targets
                .iter()
                .map(|(_, target)| *target)
                .collect(),
        }),
        D::FillArrayDataPayload(value) => Instruction::FillArrayData(FillArrayInsn {
            element_width: value.element_width,
            data: value.data.clone(),
        }),
        D::Raw { code_units } => Instruction::Raw(
            code_units
                .iter()
                .flat_map(|unit| unit.to_le_bytes())
                .collect(),
        ),
        _ => Instruction::Raw(
            reseam_dex::write::encode_instructions(std::slice::from_ref(instruction))?
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect(),
        ),
    })
}

fn opcode(instruction: &Instruction) -> Option<u16> {
    Some(match instruction {
        Instruction::Simple(value) => value.opcode,
        Instruction::Reg1(value) => value.opcode,
        Instruction::Reg2(value) => value.opcode,
        Instruction::Reg3(value) => value.opcode,
        Instruction::RegLiteral(value) => value.opcode,
        Instruction::RegString(value) => value.opcode,
        Instruction::RegType(value) => value.opcode,
        Instruction::RegField(value) => value.opcode,
        Instruction::Invoke(value) => value.opcode,
        Instruction::InvokeRange(value) => value.opcode,
        Instruction::Polymorphic(value) => value.opcode,
        Instruction::PolymorphicRange(value) => value.opcode,
        Instruction::Custom(value) => value.opcode,
        Instruction::CustomRange(value) => value.opcode,
        Instruction::RegHandle(value) => value.opcode,
        Instruction::RegProto(value) => value.opcode,
        Instruction::Branch0(value) => value.opcode,
        Instruction::Branch(value) => value.opcode,
        Instruction::Branch2(value) => value.opcode,
        Instruction::FilledArray(value) => value.opcode,
        Instruction::FilledArrayRange(value) => value.opcode,
        _ => return None,
    })
}

pub fn kotlin_to_dex(
    instruction: &Instruction,
    dex: &mut DexFile,
    dex_index: Option<usize>,
) -> reseam_dex::Result<D> {
    let lowered = super::invoke::lower(instruction, &[])?;
    let instruction = if let Some(lowered) = &lowered {
        if lowered.len() != 1 {
            return Err(invalid("instruction conversion requires an encoded invoke"));
        }
        &lowered[0]
    } else {
        instruction
    };
    if let Some(opcode) = opcode(instruction) {
        return structured(instruction, opcode, dex, dex_index);
    }
    Ok(match instruction {
        Instruction::PackedSwitchData(value) => {
            D::PackedSwitchPayload(Box::new(reseam_dex::PackedSwitchData {
                first_key: value.first_key,
                targets: value.targets.clone(),
            }))
        }
        Instruction::SparseSwitchData(value) => {
            if value.keys.len() != value.targets.len() {
                return Err(invalid(
                    "sparse switch keys and targets have different lengths",
                ));
            }
            D::SparseSwitchPayload(Box::new(reseam_dex::SparseSwitchData {
                keys_and_targets: value
                    .keys
                    .iter()
                    .copied()
                    .zip(value.targets.iter().copied())
                    .collect(),
            }))
        }
        Instruction::FillArrayData(value) => {
            if value.element_width == 0
                || !value
                    .data
                    .len()
                    .is_multiple_of(usize::from(value.element_width))
            {
                return Err(invalid("array payload does not contain complete elements"));
            }
            D::FillArrayDataPayload(Box::new(reseam_dex::FillArrayPayloadData {
                element_width: value.element_width,
                data: value.data.clone(),
            }))
        }
        Instruction::Raw(bytes) => {
            if bytes.is_empty() || !bytes.len().is_multiple_of(2) {
                return Err(invalid("raw instruction must contain complete code units"));
            }
            let mut instructions =
                reseam_dex::read::decode_instructions(bytes, 0, bytes.len() / 2)?;
            if instructions.len() != 1 {
                return Err(invalid(
                    "raw instruction must encode exactly one instruction",
                ));
            }
            instructions.remove(0)
        }
        _ => return Err(invalid("unsupported instruction representation")),
    })
}
