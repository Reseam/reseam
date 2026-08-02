// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::types::instruction::Instruction;

pub(super) fn encode_instruction(code: &mut Vec<u16>, instruction: &Instruction) -> Result<()> {
    match instruction {
        Instruction::PackedSwitchPayload(payload) => {
            let first_key = payload.first_key;
            let targets = &payload.targets;
            code.push(0x0100);
            code.push(targets.len() as u16);
            code.push(first_key as u16);
            code.push((first_key >> 16) as u16);
            for target in targets {
                code.push(*target as u16);
                code.push((*target >> 16) as u16);
            }
        }
        Instruction::SparseSwitchPayload(payload) => {
            let keys_and_targets = &payload.keys_and_targets;
            code.push(0x0200);
            code.push(keys_and_targets.len() as u16);
            for (key, _) in keys_and_targets {
                code.push(*key as u16);
                code.push((*key >> 16) as u16);
            }
            for (_, target) in keys_and_targets {
                code.push(*target as u16);
                code.push((*target >> 16) as u16);
            }
        }
        Instruction::FillArrayDataPayload(payload) => {
            let element_width = &payload.element_width;
            let data = &payload.data;
            code.push(0x0300);
            code.push(*element_width);
            let count = data.len() / *element_width as usize;
            code.push(count as u16);
            code.push((count >> 16) as u16);
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
        _ => unreachable!(),
    }
    Ok(())
}
