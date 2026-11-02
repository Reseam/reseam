// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Payloads and code units no opcode describes.

use crate::error::Result;
use crate::instruction::Instruction;
use crate::smali::body::Body;
use crate::smali::labels::LabelKind;
use crate::smali::names::{hex, long};

/// A payload as smali writes it after its directive: the directive's
/// operand, one entry per line, and the closing directive. Switch targets are
/// relative to the switch that reads the payload.
pub(super) fn payload(body: &Body<'_>, at: usize, insn: &Instruction) -> Result<String> {
    let (operand, entries, end): (String, Vec<String>, &str) = match insn {
        Instruction::PackedSwitchPayload(data) => {
            let switch = body.switch_of(at)?;
            let entries = data
                .targets
                .iter()
                .map(|&target| body.label(switch, target, LabelKind::PackedSwitch))
                .collect();
            (hex(i64::from(data.first_key)), entries, "packed-switch")
        }
        Instruction::SparseSwitchPayload(data) => {
            let switch = body.switch_of(at)?;
            let entries = data
                .keys_and_targets
                .iter()
                .map(|&(key, target)| {
                    let label = body.label(switch, target, LabelKind::SparseSwitch);
                    format!("{} -> {label}", hex(i64::from(key)))
                })
                .collect();
            (String::new(), entries, "sparse-switch")
        }
        Instruction::FillArrayDataPayload(data) => {
            let width = usize::from(data.element_width);
            let entries = data.data.chunks(width.max(1)).map(array_element).collect();
            (width.to_string(), entries, "array-data")
        }
        _ => unreachable!("only payload variants have the payload format"),
    };
    let mut text = operand;
    for entry in entries {
        text.push_str("\n    ");
        text.push_str(&entry);
    }
    text.push_str("\n.end ");
    text.push_str(end);
    Ok(text)
}

/// One element of an array payload, little endian and sign-extended, marked
/// with its width as smali marks bytes (`t`), shorts (`s`) and wide values.
fn array_element(element: &[u8]) -> String {
    let mut bytes = [0u8; 8];
    bytes[..element.len()].copy_from_slice(element);
    let shift = 64 - 8 * element.len() as u32;
    let value = (i64::from_le_bytes(bytes) << shift) >> shift;
    match element.len() {
        1 => format!("{}t", hex(value)),
        2 => format!("{}s", hex(value)),
        _ => long(value),
    }
}

/// Code units no supported opcode describes, as they are in the file.
pub(super) fn raw(_: &Body<'_>, _: usize, insn: &Instruction) -> String {
    let Instruction::Raw { code_units } = insn else {
        unreachable!("only raw instructions have the raw format");
    };
    code_units
        .iter()
        .map(|unit| format!("0x{unit:04x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
