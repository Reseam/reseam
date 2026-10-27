// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The switch and array-data payloads that a nop opcode unit introduces.

use crate::error::{Result, invalid, malformed};
use crate::instruction::opcodes::{
    FILL_ARRAY_DATA_PAYLOAD, PACKED_SWITCH_PAYLOAD, SPARSE_SWITCH_PAYLOAD,
};
use crate::instruction::{FillArrayPayloadData, Instruction, PackedSwitchData, SparseSwitchData};
use reseam_bytes::{i32_at, require_len, u16_at, u32_at};

/// The payload whose identifier starts at byte `at`, or `None` when the unit
/// there identifies no payload.
pub(super) fn decode(buf: &[u8], at: usize) -> Result<Option<Instruction>> {
    let size = || usize::from(u16_at(buf, at + 2));
    Ok(Some(match u16_at(buf, at) {
        PACKED_SWITCH_PAYLOAD => {
            let size = size();
            require_len(buf, at, 8 + size * 4, "packed-switch payload")?;
            Instruction::PackedSwitchPayload(Box::new(PackedSwitchData {
                first_key: i32_at(buf, at + 4),
                targets: (0..size).map(|i| i32_at(buf, at + 8 + i * 4)).collect(),
            }))
        }
        SPARSE_SWITCH_PAYLOAD => {
            let size = size();
            require_len(buf, at, 4 + size * 8, "sparse-switch payload")?;
            let pair = |i| {
                (
                    i32_at(buf, at + 4 + i * 4),
                    i32_at(buf, at + 4 + size * 4 + i * 4),
                )
            };
            Instruction::SparseSwitchPayload(Box::new(SparseSwitchData {
                keys_and_targets: (0..size).map(pair).collect(),
            }))
        }
        FILL_ARRAY_DATA_PAYLOAD => {
            let bytes = (u32_at(buf, at + 4) as usize)
                .checked_mul(size())
                .ok_or_else(|| {
                    malformed("fill-array-data payload", at, "payload size overflowed")
                })?;
            require_len(buf, at, 8 + bytes, "fill-array-data payload")?;
            Instruction::FillArrayDataPayload(Box::new(FillArrayPayloadData {
                element_width: u16_at(buf, at + 2),
                data: buf[at + 8..at + 8 + bytes].to_vec(),
            }))
        }
        _ => return Ok(None),
    }))
}

pub(super) fn encode(code: &mut Vec<u16>, insn: &Instruction) -> Result<()> {
    match insn {
        Instruction::PackedSwitchPayload(payload) => {
            let size = u16::try_from(payload.targets.len())
                .map_err(|_| invalid("packed-switch payload", "too many targets"))?;
            code.extend([PACKED_SWITCH_PAYLOAD, size]);
            code.extend(split32(payload.first_key as u32));
            code.extend(
                payload
                    .targets
                    .iter()
                    .flat_map(|&target| split32(target as u32)),
            );
        }
        Instruction::SparseSwitchPayload(payload) => {
            let pairs = &payload.keys_and_targets;
            let size = u16::try_from(pairs.len())
                .map_err(|_| invalid("sparse-switch payload", "too many keys"))?;
            code.extend([SPARSE_SWITCH_PAYLOAD, size]);
            code.extend(pairs.iter().flat_map(|&(key, _)| split32(key as u32)));
            code.extend(pairs.iter().flat_map(|&(_, target)| split32(target as u32)));
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
            code.extend([FILL_ARRAY_DATA_PAYLOAD, payload.element_width]);
            code.extend(split32(count));
            let (pairs, tail) = payload.data.as_chunks::<2>();
            code.extend(pairs.iter().map(|&pair| u16::from_le_bytes(pair)));
            code.extend(tail.iter().map(|&last| u16::from(last)));
        }
        _ => return Err(invalid("instruction encoding", "expected a payload")),
    }
    Ok(())
}

/// A 32-bit value as its low code unit, then its high one.
pub(super) fn split32(value: u32) -> [u16; 2] {
    [value as u16, (value >> 16) as u16]
}
