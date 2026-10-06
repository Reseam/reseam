// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;
use reseam_apk::reseam_dex::types::code_rewrite::InstructionExpansion;
use reseam_apk::reseam_dex::{
    AccessFlags, CodeItem, Instruction as DexInsn, MethodIdx, RegList,
    find_contiguous_free_registers,
};

use crate::kotlin::convert::kotlin_to_dex;
use crate::kotlin::handles::{code_mut, method_mut, with_method_mut};
use crate::kotlin::link::{link_instructions, link_method};
use crate::kotlin::types::{Instruction, MethodEdit, MethodRef};

use super::logged;

fn edit_code<R>(
    m: u32,
    insns: &[Instruction],
    f: impl FnOnce(&mut CodeItem, Vec<DexInsn>) -> Result<R, String>,
) -> Result<R, String> {
    link_instructions(insns);
    let result = with_method_mut(m, |dex, loc| {
        let insns = logged(
            "convert instructions",
            insns
                .iter()
                .map(|insn| kotlin_to_dex(insn, dex, Some(loc.dex_idx)))
                .collect(),
        )?;
        let code = logged("access method", code_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} has no code"))?;
        f(code, insns).map(Some)
    });
    crate::kotlin::handles::check_call()?;
    result.ok_or_else(|| format!("method handle {m} could not be edited"))
}

#[export]
pub fn set_instructions(m: u32, insns: Vec<Instruction>) -> Result<(), String> {
    edit_code(m, &insns, |code, insns| {
        code.set_instructions(insns);
        Ok(())
    })
}

#[export]
pub fn replace_body(
    m: u32,
    registers_size: u16,
    outs_size: u16,
    insns: Vec<Instruction>,
) -> Result<(), String> {
    link_instructions(&insns);
    let outcome = with_method_mut(m, |dex, loc| {
        let method = logged("access method", method_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} is missing"))?;
        let (id, flags) = (method.method, method.access_flags);
        let proto = dex.proto_descriptor(&dex.proto(dex.method_id(id).proto));
        let incoming = logged(
            "replacement frame",
            crate::kotlin::invoke::incoming_words(&proto, flags),
        )?;
        let code = logged(
            "replace body",
            super::assembly::assemble(insns, Vec::new(), dex, Some(loc.dex_idx), |code, _| {
                code.set_register_frame(registers_size, incoming, outs_size)
            }),
        )?
        .code;
        let method = logged("replace method", method_mut(dex, loc))?
            .expect("assembly leaves the method slot in place");
        method.code = Some(code);
        method
            .access_flags
            .remove(AccessFlags::NATIVE | AccessFlags::ABSTRACT);
        Ok(Some(()))
    });
    crate::kotlin::handles::check_call()?;
    outcome.ok_or_else(|| format!("method handle {m} could not be replaced"))
}

pub(super) fn edit_mapping(
    mapping: reseam_apk::reseam_dex::types::code_rewrite::InstructionMap,
) -> Result<MethodEdit, String> {
    fn indices(values: &[usize]) -> Result<Vec<u32>, String> {
        values
            .iter()
            .map(|index| {
                u32::try_from(*index).map_err(|_| "instruction index exceeds 32 bits".into())
            })
            .collect()
    }
    Ok(MethodEdit {
        register_shift: 0,
        starts: indices(mapping.starts())?,
        instructions: indices(mapping.instructions())?,
        ends: indices(mapping.ends())?,
    })
}

#[export]
pub fn insert_instructions(
    m: u32,
    index: u32,
    insns: Vec<Instruction>,
) -> Result<MethodEdit, String> {
    edit_code(m, &insns, |code, insns| {
        edit_mapping(logged(
            "insert instructions",
            code.insert_instructions(index as usize, &insns),
        )?)
    })
}

#[export]
pub fn insert_before_instruction(
    m: u32,
    index: u32,
    insns: Vec<Instruction>,
) -> Result<MethodEdit, String> {
    edit_code(m, &insns, |code, insns| {
        let mut expansions: Vec<InstructionExpansion> = code
            .instructions()
            .iter()
            .cloned()
            .map(Into::into)
            .collect();
        let expansion = expansions
            .get_mut(index as usize)
            .ok_or_else(|| format!("instruction index {index} outside body"))?;
        expansion.before = insns;
        edit_mapping(logged(
            "insert before instruction",
            code.rewrite_instructions(expansions, &std::collections::BTreeMap::new()),
        )?)
    })
}

#[export]
pub fn replace_instruction(
    m: u32,
    index: u32,
    insns: Vec<Instruction>,
) -> Result<MethodEdit, String> {
    edit_code(m, &insns, |code, insns| {
        edit_mapping(logged(
            "replace instruction",
            code.replace_instructions(index as usize, &insns),
        )?)
    })
}

#[export]
pub fn remove_instructions(m: u32, index: u32, count: u32) -> Result<MethodEdit, String> {
    let result = with_method_mut(m, |dex, loc| {
        let code = logged("access method", code_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} has no code"))?;
        edit_mapping(logged(
            "remove instructions",
            code.remove_instructions(index as usize, count as usize),
        )?)
        .map(Some)
    });
    crate::kotlin::handles::check_call()?;
    result.ok_or_else(|| format!("method handle {m} could not be edited"))
}

#[export]
pub fn return_early(m: u32) {
    with_method_mut(m, |dex, loc| {
        Ok(logged("access method", method_mut(dex, loc))?
            .map(reseam_apk::reseam_dex::EncodedMethod::return_early))
    });
}

#[export]
pub fn return_early_int(m: u32, value: i32) {
    with_method_mut(m, |dex, loc| {
        Ok(logged("access method", method_mut(dex, loc))?
            .map(|method| method.return_early_int(value)))
    });
}

#[export]
pub fn return_early_object_null(m: u32) {
    with_method_mut(m, |dex, loc| {
        Ok(logged("access method", method_mut(dex, loc))?
            .map(|method| method.return_early_object(0)))
    });
}

#[export]
pub fn return_early_wide(m: u32, value: i64) {
    with_method_mut(m, |dex, loc| {
        Ok(logged("access method", method_mut(dex, loc))?
            .map(|method| method.return_early_wide(value)))
    });
}

#[export]
pub fn insert_invoke_static(
    m: u32,
    index: u32,
    class_name: String,
    name: String,
    proto: String,
    registers: Vec<u16>,
) -> bool {
    insert_invoke(m, index, &class_name, &name, &proto, &registers, None)
}

#[export]
#[expect(
    clippy::too_many_arguments,
    reason = "the legacy JNI export retains its positional arguments"
)]
pub fn insert_invoke_static_with_move_result(
    m: u32,
    index: u32,
    class_name: String,
    name: String,
    proto: String,
    registers: Vec<u16>,
    result_register: u16,
    is_object: bool,
) -> bool {
    insert_invoke(
        m,
        index,
        &class_name,
        &name,
        &proto,
        &registers,
        Some((result_register, is_object)),
    )
}

fn insert_invoke(
    m: u32,
    index: u32,
    class: &str,
    name: &str,
    proto: &str,
    registers: &[u16],
    move_result: Option<(u16, bool)>,
) -> bool {
    link_method(&MethodRef {
        defining_class: class.to_owned(),
        name: name.to_owned(),
        proto: proto.to_owned(),
    });
    with_method_mut(m, |dex, loc| {
        let method = logged("intern method", dex.intern_method(class, name, proto))?;
        let Some(code) = logged("access method", code_mut(dex, loc))? else {
            return Ok(None);
        };
        let mut lowered = lower_static_invoke(code, index as usize, method, proto, registers)?;
        if let Some((dest, is_object)) = move_result {
            let dest = u8::try_from(dest)
                .map_err(|_| format!("move-result destination {dest} exceeds v255"))?;
            lowered.push(if is_object {
                DexInsn::MoveResultObject { dest }
            } else {
                DexInsn::MoveResult { dest }
            });
        }
        logged(
            "insert_invoke_static",
            code.insert_instructions(index as usize, &lowered),
        )
        .map(Some)
    })
    .is_some()
}

fn lower_static_invoke(
    code: &CodeItem,
    index: usize,
    method: MethodIdx,
    proto: &str,
    registers: &[u16],
) -> Result<Vec<DexInsn>, String> {
    let input = Instruction::Invoke(crate::kotlin::types::InvokeInsn {
        opcode: 0x71,
        registers: registers.to_vec(),
        method: MethodRef {
            defining_class: "Ljava/lang/Object;".into(),
            name: "call".into(),
            proto: proto.into(),
        },
    });
    let words = crate::kotlin::invoke::scratch_words(&input).map_err(|error| error.to_string())?;
    let scratch = if words == 0 {
        Vec::new()
    } else {
        find_contiguous_free_registers(code, index, words, registers)
            .ok_or_else(|| format!("no contiguous scratch span at instruction {index}"))?
    };
    let lowered = crate::kotlin::invoke::lower_instruction(input, scratch)?;
    lowered
        .iter()
        .map(|instruction| match instruction {
            Instruction::Invoke(value) => Ok(DexInsn::InvokeStatic {
                method,
                args: RegList::try_from_iter(
                    value.registers.iter().map(|register| *register as u8),
                )
                .map_err(|error| error.to_string())?,
            }),
            Instruction::InvokeRange(value) => Ok(DexInsn::InvokeStaticRange {
                method,
                first_reg: value.start_reg,
                count: value.reg_count as u8,
            }),
            Instruction::Reg2(value) => {
                // Typed moves have no pool operands.
                let mut empty =
                    reseam_apk::reseam_dex::DexFile::new(reseam_apk::reseam_dex::DexHeader::new(
                        reseam_apk::reseam_dex::DexVersion::V035,
                    ));
                kotlin_to_dex(&Instruction::Reg2(*value), &mut empty, None)
                    .map_err(|error| error.to_string())
            }
            _ => Err("invoke lowering produced an unsupported instruction".into()),
        })
        .collect()
}
