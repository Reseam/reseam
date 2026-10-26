// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use crate::bytes::align;
use crate::edit::{EditedFunction, FunctionBody};
use crate::error::{Result, Unsupported, invalid, unsupported};
use crate::function::ExceptionHandler;
use crate::function::Function;
use crate::opcode::{Instruction, Opcode, OperandKind};
use crate::switch::switch_table;

/// Moves `inst` to its narrowest encoding that holds its operands, if one does. Relocated
/// branches always take 32-bit addresses, since their distances change.
fn promote(inst: &mut Instruction) -> Option<()> {
    let operands = inst.definition().operands;
    for (value, operand) in inst.values.iter_mut().zip(operands) {
        if operand.kind == OperandKind::Addr8 {
            *value = u64::from(i32::from(*value as i8).cast_unsigned());
        }
    }
    let fits = |opcode: &Opcode| {
        opcode.operands.iter().zip(&inst.values).all(|(o, &v)| {
            o.kind != OperandKind::Addr8 && (o.kind.width() == 8 || v < 1 << (o.kind.width() * 8))
        })
    };
    let op = inst.encodings().find(|&opcode| fits(opcode))?.op;
    inst.op = op;
    Some(())
}

pub(crate) fn encode(instructions: &[Instruction]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for inst in instructions {
        bytes.push(inst.code());
        for (operand, value) in inst.operands() {
            bytes.extend_from_slice(&value.to_le_bytes()[..operand.kind.width()]);
        }
    }
    bytes
}

/// Promotes narrow operands and relocates branch targets, switch tables and
/// exception handlers together. `strings` remaps string switch keys.
pub(crate) fn assemble(
    function: &Function<'_>,
    mut instructions: Vec<Instruction>,
    strings: Option<&[u32]>,
) -> Result<EditedFunction> {
    for inst in &mut instructions {
        promote(inst).ok_or_else(|| unsupported(function.id, Unsupported::Encoding))?;
    }
    let mut locations = BTreeMap::new();
    let mut cursor = 0_u32;
    for inst in &instructions {
        if let Some(offset) = inst.offset {
            locations.entry(offset).or_insert(cursor);
        }
        cursor += inst.definition().size() as u32;
    }
    locations.insert(function.header.size, cursor);
    let code_size = cursor;
    let start = function.header.offset as usize;
    let moved = |old: i64| -> Result<u32> {
        u32::try_from(old)
            .ok()
            .and_then(|old| locations.get(&old).copied())
            .ok_or_else(|| {
                invalid(
                    start.saturating_add_signed(old as isize),
                    "branch target is not an instruction boundary",
                )
            })
    };
    let mut payload = Vec::new();
    let mut position = 0_u32;
    for inst in &mut instructions {
        let definition = inst.definition();
        for (operand, relative) in inst.jumps().collect::<Vec<_>>() {
            let origin = inst.offset.expect("assembled branches are decoded");
            let target = moved(i64::from(origin) + relative)?;
            inst.values[operand] = u64::from(target.wrapping_sub(position));
        }
        if let Some(table) = switch_table(function, inst)? {
            let origin = i64::from(
                inst.offset
                    .expect("switch tables belong to decoded instructions"),
            );
            let new_table = align(code_size as usize + payload.len());
            payload.resize(new_table - code_size as usize, 0);
            inst.values[table.operand] = new_table as u64 - u64::from(position);
            for (key, target) in table.entries() {
                if let Some(key) = key {
                    let key = match strings {
                        Some(strings) => *strings.get(key.0 as usize).ok_or_else(|| {
                            invalid(table.offset, "switch string ID out of range")
                        })?,
                        None => key.0,
                    };
                    payload.extend_from_slice(&key.to_le_bytes());
                }
                let new_target = moved(origin + i64::from(target))?;
                payload.extend_from_slice(&new_target.wrapping_sub(position).to_le_bytes());
            }
        }
        position += definition.size() as u32;
    }
    let mut body = encode(&instructions);
    body.extend_from_slice(&payload);
    let exceptions = function
        .exception_handlers()?
        .into_iter()
        .map(|handler| {
            Ok(ExceptionHandler {
                start: moved(handler.start.into())?,
                end: moved(handler.end.into())?,
                target: moved(handler.target.into())?,
            })
        })
        .collect::<Result<_>>()?;
    let mut header = function.header.clone();
    header.size = code_size;
    Ok(EditedFunction::new(
        header,
        FunctionBody::Owned(body),
        exceptions,
    ))
}
