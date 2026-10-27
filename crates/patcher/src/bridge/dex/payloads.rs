// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Switch and array payloads and raw code, which cross the bridge as data
//! rather than as structured instructions.

use reseam_dex::Instruction as D;

use super::convert::invalid;
use super::types::{FillArrayInsn, Instruction, PackedSwitchInsn, SparseSwitchInsn};

pub(in crate::bridge) fn export_payload(instruction: &D) -> reseam_dex::Result<Instruction> {
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
            reseam_dex::encode_instructions(std::slice::from_ref(instruction))?
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect(),
        ),
    })
}

pub(in crate::bridge) fn import_payload(instruction: &Instruction) -> reseam_dex::Result<D> {
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
            let mut instructions = reseam_dex::decode_instructions(bytes, 0, bytes.len() / 2)?;
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
