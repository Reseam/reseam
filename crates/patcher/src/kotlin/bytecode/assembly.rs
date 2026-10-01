// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use boltffi::export;
use dex::types::code_rewrite::{BranchDestination, branch_offset as native_branch_offset};
use reseam_apk::reseam_dex::{self as dex, CodeItem, DexFile};

use crate::kotlin::convert::{dex_to_kotlin, kotlin_to_dex};
use crate::kotlin::types::{Instruction, ScratchSpan};

pub(super) struct Assembly {
    pub code: CodeItem,
    pub boundaries: Vec<usize>,
    originals: Vec<Vec<Instruction>>,
}

fn invalid(reason: impl Into<String>) -> dex::DexError {
    dex::DexError::Invalid {
        section: "instruction block",
        reason: reason.into(),
    }
}

fn branch_offset(instruction: &Instruction) -> Option<i32> {
    match instruction {
        Instruction::Branch0(value) => Some(value.offset),
        Instruction::Branch(value) if (0x38..=0x3d).contains(&value.opcode) => Some(value.offset),
        Instruction::Branch2(value) => Some(value.offset),
        _ => None,
    }
}

fn branch_placeholder(instruction: &Instruction) -> Option<Instruction> {
    Some(match instruction {
        Instruction::Branch0(value) => {
            let mut value = *value;
            value.offset = 0;
            Instruction::Branch0(value)
        }
        Instruction::Branch(value) if (0x38..=0x3d).contains(&value.opcode) => {
            let mut value = *value;
            value.offset = 0;
            Instruction::Branch(value)
        }
        Instruction::Branch2(value) => {
            let mut value = *value;
            value.offset = 0;
            Instruction::Branch2(value)
        }
        _ => return None,
    })
}

pub(super) fn assemble(
    instructions: Vec<Instruction>,
    scratch: Vec<ScratchSpan>,
    dex: &mut DexFile,
    dex_index: Option<usize>,
    initialize: impl FnOnce(&mut CodeItem, &mut DexFile) -> dex::Result<()>,
) -> dex::Result<Assembly> {
    let mut scratch_by_instruction = BTreeMap::new();
    for span in scratch {
        let index = span.instruction_index as usize;
        if index >= instructions.len()
            || scratch_by_instruction
                .insert(index, span.registers)
                .is_some()
        {
            return Err(invalid("scratch hint must name one existing instruction"));
        }
    }
    let mut originals = Vec::with_capacity(instructions.len());
    let mut native = Vec::with_capacity(instructions.len());
    let mut offsets = Vec::with_capacity(instructions.len() + 1);
    let mut branches = Vec::new();
    let mut address = 0u32;
    for (index, instruction) in instructions.into_iter().enumerate() {
        offsets.push(address);
        let authored_branch = branch_offset(&instruction);
        let lowered = crate::kotlin::invoke::lower_instruction(
            instruction,
            scratch_by_instruction.remove(&index).unwrap_or_default(),
        )
        .map_err(invalid)?;
        let primary = lowered
            .last()
            .expect("lowering preserves each original instruction");
        let placeholder = branch_placeholder(primary);
        let primary = kotlin_to_dex(placeholder.as_ref().unwrap_or(primary), dex, dex_index)?;
        branches.push(authored_branch.or_else(|| native_branch_offset(&primary)));
        address = address
            .checked_add(primary.code_units())
            .ok_or_else(|| invalid("code unit count exceeds 32 bits"))?;
        native.push(primary);
        originals.push(lowered);
    }
    offsets.push(address);
    let targets = branches
        .into_iter()
        .enumerate()
        .filter_map(|(index, delta)| delta.map(|delta| (index, delta)))
        .map(|(index, delta)| {
            let target = i64::from(offsets[index]) + i64::from(delta);
            let destination = u32::try_from(target)
                .ok()
                .and_then(|target| offsets.binary_search(&target).ok());
            Ok((
                index,
                match destination {
                    Some(target) => BranchDestination::Instruction(target),
                    None if (0..=i64::from(address)).contains(&target) => {
                        return Err(invalid("branch target is not an instruction boundary"));
                    }
                    None => {
                        native[index] = kotlin_to_dex(
                            originals[index]
                                .last()
                                .expect("lowering retains every original instruction"),
                            dex,
                            dex_index,
                        )?;
                        BranchDestination::External
                    }
                },
            ))
        })
        .collect::<dex::Result<BTreeMap<_, _>>>()?;
    let prefixes = originals
        .iter()
        .map(|lowered| {
            lowered[..lowered.len() - 1]
                .iter()
                .map(|instruction| kotlin_to_dex(instruction, dex, dex_index))
                .collect::<dex::Result<Vec<_>>>()
        })
        .collect::<dex::Result<_>>()?;
    let mut code = CodeItem::new(u16::MAX, 0, 0, native)?;
    initialize(&mut code, dex)?;
    let (code, mapping) = code.prepend_instructions(prefixes, &targets)?;
    let boundaries = mapping.starts().to_vec();
    Ok(Assembly {
        code,
        boundaries,
        originals,
    })
}

impl Assembly {
    fn into_sdk(self, dex: &DexFile) -> dex::Result<Vec<Instruction>> {
        let mut unchanged = BTreeMap::new();
        for (index, lowered) in self.originals.into_iter().enumerate() {
            for (offset, instruction) in lowered.into_iter().enumerate() {
                let native = &self.code.instructions()[self.boundaries[index] + offset];
                // Payload identifiers span two bytes; data references must retain relocated offsets.
                if native_branch_offset(native).is_none()
                    && !matches!(
                        native,
                        dex::Instruction::FillArrayData { .. }
                            | dex::Instruction::PackedSwitch { .. }
                            | dex::Instruction::SparseSwitch { .. }
                    )
                    && native
                        .opcode()
                        .is_some_and(|opcode| u8::try_from(opcode).is_ok())
                {
                    unchanged.insert(self.boundaries[index] + offset, instruction);
                }
            }
        }
        self.code
            .instructions()
            .iter()
            .enumerate()
            .map(|(index, instruction)| match unchanged.remove(&index) {
                Some(instruction) => Ok(instruction),
                None => dex_to_kotlin(instruction, dex, None),
            })
            .collect()
    }
}

#[export]
pub fn lower_instructions(
    instructions: Vec<Instruction>,
    scratch: Vec<ScratchSpan>,
) -> Result<Vec<Instruction>, String> {
    let mut dex = DexFile::new(dex::DexHeader::new(dex::DexVersion::V039));
    assemble(instructions, scratch, &mut dex, None, |_, _| Ok(()))
        .and_then(|assembly| assembly.into_sdk(&dex))
        .map_err(|error| error.to_string())
}
