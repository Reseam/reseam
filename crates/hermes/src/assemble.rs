use std::collections::BTreeMap;

use crate::Function;
use crate::edit::{EditedFunction, FunctionBody};
use crate::error::{Result, invalid};
use crate::opcode::{BytecodeVersion, Instruction, OperandKind};
use crate::parse::{align, read_u32, slice, switch_table};

pub(crate) fn instruction(name: &str, values: &[u64]) -> Instruction {
    let version = BytecodeVersion::V98;
    let opcode = version
        .opcodes()
        .iter()
        .position(|opcode| opcode.name == name)
        .expect("internal opcode names exist in the generated table") as u8;
    Instruction {
        version,
        offset: u32::MAX,
        opcode,
        values: values.to_vec(),
    }
}

fn signed(value: u64, kind: OperandKind) -> i64 {
    match kind {
        OperandKind::Addr8 => i64::from(value as i8),
        OperandKind::Addr32 | OperandKind::Imm32 => i64::from(value as i32),
        _ => value as i64,
    }
}

fn promote(inst: &mut Instruction) -> Result<()> {
    let definition = inst.definition();
    if definition
        .operands
        .iter()
        .any(|o| o.kind == OperandKind::Addr8)
    {
        let name = format!("{}Long", definition.name);
        for (value, operand) in inst.values.iter_mut().zip(definition.operands) {
            if operand.kind == OperandKind::Addr8 {
                *value = u64::from(i32::from(*value as i8) as u32);
            }
        }
        inst.opcode = inst
            .version
            .opcodes()
            .iter()
            .position(|o| o.name == name)
            .ok_or_else(|| invalid(inst.offset as usize, "no long jump variant"))?
            as u8;
    }
    let definition = inst.definition();
    let fits = |opcode: &crate::opcode::Opcode| {
        opcode.operands.len() == inst.values.len()
            && opcode
                .operands
                .iter()
                .zip(&inst.values)
                .all(|(o, &v)| o.kind.width() == 8 || v < 1_u64 << (o.kind.width() * 8))
    };
    if fits(definition) {
        return Ok(());
    }
    let base = definition
        .name
        .strip_suffix("Short")
        .unwrap_or(definition.name);
    let names = [format!("{base}Long"), format!("{base}LongIndex")];
    let code = inst
        .version
        .opcodes()
        .iter()
        .enumerate()
        .find(|(_, opcode)| names.iter().any(|n| n == opcode.name) && fits(opcode))
        .ok_or_else(|| {
            invalid(
                inst.offset as usize,
                format!("operands do not fit {}", definition.name),
            )
        })?;
    inst.opcode = code.0 as u8;
    Ok(())
}

pub(crate) fn encode(instructions: &[Instruction]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for inst in instructions {
        bytes.push(inst.opcode);
        for (operand, value) in inst.definition().operands.iter().zip(&inst.values) {
            bytes.extend_from_slice(&value.to_le_bytes()[..operand.kind.width()]);
        }
    }
    bytes
}

/// Promotes narrow operands and relocates branch targets, switch tables and
/// exception handlers together. New instructions use offset `u32::MAX`.
pub(crate) fn assemble(
    function: &Function<'_>,
    mut instructions: Vec<Instruction>,
    strings: Option<&[u32]>,
) -> Result<EditedFunction> {
    for inst in &mut instructions {
        promote(inst)?;
    }
    let mut locations = BTreeMap::new();
    let mut cursor = 0_u32;
    for inst in &instructions {
        if inst.offset != u32::MAX {
            locations.entry(inst.offset).or_insert(cursor);
        }
        cursor += inst.definition().size() as u32;
    }
    locations.insert(function.header.size, cursor);
    let code_size = cursor;
    let moved = |old: i64| -> Result<u32> {
        let old = u32::try_from(old).map_err(|_| invalid(0, "negative branch target"))?;
        locations
            .get(&old)
            .copied()
            .ok_or_else(|| invalid(old as usize, "branch target is not an instruction boundary"))
    };
    let mut payload = Vec::new();
    let mut position = 0_u32;
    for inst in &mut instructions {
        let definition = inst.definition();
        for (operand, value) in definition.operands.iter().zip(&mut inst.values) {
            if matches!(operand.kind, OperandKind::Addr8 | OperandKind::Addr32) {
                if inst.offset == u32::MAX {
                    return Err(invalid(0, "generated branch lacks a target"));
                }
                let target = moved(i64::from(inst.offset) + signed(*value, operand.kind))?;
                *value = u64::from(target.wrapping_sub(position));
            }
        }
        if let Some(table) = switch_table(function, inst)? {
            let new_table = align(code_size as usize + payload.len());
            payload.resize(new_table - code_size as usize, 0);
            inst.values[table.operand] = new_table as u64 - u64::from(position);
            for entry in table.bytes.chunks_exact(table.stride) {
                let target_offset = if table.stride == 8 {
                    let id = read_u32(entry, 0)?;
                    let remapped = if let Some(strings) = strings {
                        *strings
                            .get(id as usize)
                            .ok_or_else(|| invalid(table.offset, "switch string ID out of range"))?
                    } else {
                        id
                    };
                    payload.extend_from_slice(&remapped.to_le_bytes());
                    4
                } else {
                    0
                };
                let target = read_u32(entry, target_offset)? as i32;
                let new_target = moved(i64::from(inst.offset) + i64::from(target))?;
                payload.extend_from_slice(&new_target.wrapping_sub(position).to_le_bytes());
            }
        }
        position += definition.size() as u32;
    }
    let mut body = encode(&instructions);
    body.extend_from_slice(&payload);
    let mut exceptions = Vec::new();
    if function.header.flags & 8 != 0 {
        let count = read_u32(function.source, function.header.info_offset)?;
        for index in 0..count as usize {
            let entry = slice(
                function.source,
                function.header.info_offset + 4 + index * 12,
                12,
            )?;
            exceptions.push([
                moved(i64::from(read_u32(entry, 0)?))?,
                moved(i64::from(read_u32(entry, 4)?))?,
                moved(i64::from(read_u32(entry, 8)?))?,
            ]);
        }
    }
    let mut header = function.header.clone();
    header.size = code_size;
    header.flags &= !0x30;
    Ok(EditedFunction {
        header,
        body: FunctionBody::Owned(body),
        exceptions,
    })
}
