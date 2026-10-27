// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use reseam_dex::{self as dex, BranchDestination, CodeItem, DexFile, InstructionExpansion};

use crate::bridge::dex::convert::kotlin_to_dex;
use crate::bridge::dex::types::Instruction;

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

/// A body from instructions as a patch wrote them. Each branch keeps the instruction its
/// displacement names, or the outside of the block, while its encoding widens to reach it.
pub(super) fn assemble(
    instructions: Vec<Instruction>,
    dex: &mut DexFile,
    dex_index: Option<usize>,
    initialize: impl FnOnce(&mut CodeItem, &mut DexFile) -> dex::Result<()>,
) -> dex::Result<CodeItem> {
    let mut native = Vec::with_capacity(instructions.len());
    let mut offsets = Vec::with_capacity(instructions.len() + 1);
    let mut branches = Vec::new();
    let mut address = 0u32;
    for instruction in &instructions {
        offsets.push(address);
        let placeholder = branch_placeholder(instruction);
        let converted = kotlin_to_dex(placeholder.as_ref().unwrap_or(instruction), dex, dex_index)?;
        branches.push(branch_offset(instruction).or_else(|| converted.branch_offset()));
        address = address
            .checked_add(converted.code_units())
            .ok_or_else(|| invalid("code unit count exceeds 32 bits"))?;
        native.push(converted);
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
                        native[index] = kotlin_to_dex(&instructions[index], dex, dex_index)?;
                        BranchDestination::External
                    }
                },
            ))
        })
        .collect::<dex::Result<BTreeMap<_, _>>>()?;
    let expansions = native
        .iter()
        .cloned()
        .map(InstructionExpansion::from)
        .collect();
    let mut code = CodeItem::new(u16::MAX, 0, 0, native)?;
    initialize(&mut code, dex)?;
    code.rewrite_instructions(expansions, &targets)?;
    Ok(code)
}
