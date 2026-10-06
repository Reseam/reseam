// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, invalid};
use crate::types::instruction::Instruction;
use crate::types::instruction_encoding::opcodes::{
    FILL_ARRAY_DATA_PAYLOAD, PACKED_SWITCH_PAYLOAD, SPARSE_SWITCH_PAYLOAD,
};

pub(super) fn encode_instruction(code: &mut Vec<u16>, instruction: &Instruction) -> Result<()> {
    match instruction {
        Instruction::PackedSwitchPayload(payload) => {
            let first_key = payload.first_key;
            let targets = &payload.targets;
            code.push(PACKED_SWITCH_PAYLOAD);
            code.push(
                u16::try_from(targets.len())
                    .map_err(|_| invalid("packed-switch payload", "too many targets"))?,
            );
            code.push(first_key as u16);
            code.push((first_key >> 16) as u16);
            for target in targets {
                code.push(*target as u16);
                code.push((*target >> 16) as u16);
            }
        }
        Instruction::SparseSwitchPayload(payload) => {
            let keys_and_targets = &payload.keys_and_targets;
            code.push(SPARSE_SWITCH_PAYLOAD);
            code.push(
                u16::try_from(keys_and_targets.len())
                    .map_err(|_| invalid("sparse-switch payload", "too many keys"))?,
            );
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
            let width = usize::from(payload.element_width);
            if width == 0 || !payload.data.len().is_multiple_of(width) {
                return Err(invalid(
                    "fill-array payload",
                    "data must contain complete elements with nonzero width",
                ));
            }
            let count = u32::try_from(payload.data.len() / width)
                .map_err(|_| invalid("fill-array payload", "element count exceeds u32"))?;
            code.extend([
                FILL_ARRAY_DATA_PAYLOAD,
                payload.element_width,
                count as u16,
                (count >> 16) as u16,
            ]);
            let (pairs, tail) = payload.data.as_chunks::<2>();
            code.extend(pairs.iter().map(|&pair| u16::from_le_bytes(pair)));
            if let Some(&last) = tail.first() {
                code.push(u16::from(last));
            }
        }
        Instruction::Raw { code_units } => {
            code.extend_from_slice(code_units);
        }
        _ => {
            return Err(invalid(
                "instruction encoding",
                "instruction is outside the encoded family",
            ));
        }
    }
    Ok(())
}
