// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Coarse verifier types for choosing moves at polymorphic equality operands.
//! This distinguishes references, scalar values, and wide pairs, not classes.

use super::code::CodeItem;
use super::instruction::Instruction;
use super::register_analysis::ControlFlow;
use super::register_operands::{map_operands, Access, RegisterKind};
use crate::error::{invalid, Result};

const UNDEFINED: u8 = 1;
const VALUE: u8 = 2;
const OBJECT: u8 = 4;
const ZERO: u8 = 8;
const WIDE: u8 = 16;
const HIGH: u8 = 32;

pub(crate) struct RegisterTypes {
    before: Vec<Option<Vec<u8>>>,
}

impl RegisterTypes {
    pub fn new(code: &CodeItem, incoming: &[RegisterKind]) -> Result<Self> {
        let graph = ControlFlow::new(code)
            .ok_or_else(|| invalid("register types", "invalid control flow"))?;
        let mut before = vec![None; code.instructions.len()];
        if before.is_empty() {
            return Ok(Self { before });
        }
        let base = code
            .registers_size
            .checked_sub(code.ins_size)
            .ok_or_else(|| invalid("register types", "parameters exceed frame"))?;
        let mut entry = vec![UNDEFINED; code.registers_size as usize];
        let mut register = base as usize;
        for &kind in incoming {
            write(&mut entry, register, kind)?;
            register += kind.words() as usize;
        }
        if register != entry.len() {
            return Err(invalid(
                "register types",
                "parameter width does not match ins_size",
            ));
        }
        before[0] = Some(entry);
        let mut queue = std::collections::VecDeque::from([0]);
        let mut queued = vec![false; before.len()];
        queued[0] = true;
        while let Some(index) = queue.pop_front() {
            queued[index] = false;
            let input = before[index].as_ref().unwrap().clone();
            let mut output = input.clone();
            let insn = &code.instructions[index];
            let source = match insn {
                Instruction::Move { src, .. } | Instruction::MoveObject { src, .. } => {
                    Some(u16::from(*src))
                }
                Instruction::MoveFrom16 { src, .. }
                | Instruction::MoveObjectFrom16 { src, .. }
                | Instruction::Move16 { src, .. }
                | Instruction::MoveObject16 { src, .. } => Some(*src),
                _ => None,
            };
            let zero = matches!(
                insn,
                Instruction::Const4 { value: 0, .. }
                    | Instruction::Const16 { value: 0, .. }
                    | Instruction::Const { value: 0, .. }
                    | Instruction::ConstHigh16 { value: 0, .. }
            );
            map_operands(&mut insn.clone(), |operand| {
                if operand.access != Access::Read {
                    write(&mut output, operand.register as usize, operand.kind)?;
                    if let Some(source) = source {
                        output[operand.register as usize] =
                            *input.get(source as usize).ok_or_else(|| {
                                invalid("register types", "move source outside frame")
                            })?;
                    } else if zero {
                        output[operand.register as usize] = ZERO;
                    }
                }
                Ok(operand.register)
            })?;
            for (targets, state) in [
                (&graph.successors[index], &output),
                (&graph.handlers[index], &input),
            ] {
                for &next in targets {
                    let changed = if let Some(previous) = &mut before[next] {
                        let mut changed = false;
                        for (previous, &value) in previous.iter_mut().zip(state) {
                            let combined = *previous | value;
                            changed |= combined != *previous;
                            *previous = combined;
                        }
                        changed
                    } else {
                        before[next] = Some(state.clone());
                        true
                    };
                    if changed && !queued[next] {
                        queue.push_back(next);
                        queued[next] = true;
                    }
                }
            }
        }
        Ok(Self { before })
    }

    pub fn kind(&self, index: usize, register: u16) -> Result<RegisterKind> {
        let bits = self
            .before
            .get(index)
            .and_then(Option::as_ref)
            .and_then(|state| state.get(register as usize))
            .copied()
            .unwrap_or(UNDEFINED);
        if bits & !(OBJECT | ZERO) == 0 && bits & OBJECT != 0 {
            return Ok(RegisterKind::Object);
        }
        if bits & !(VALUE | ZERO) == 0 {
            return Ok(RegisterKind::Value);
        }
        Err(invalid(
            "register types",
            format!("cannot determine move type for v{register} at instruction {index}"),
        ))
    }
}

fn write(state: &mut [u8], register: usize, kind: RegisterKind) -> Result<()> {
    let words = state
        .get_mut(register..register + kind.words() as usize)
        .ok_or_else(|| invalid("register types", "operand outside frame"))?;
    match kind {
        RegisterKind::Value => words[0] = VALUE,
        RegisterKind::Object => words[0] = OBJECT,
        RegisterKind::Wide => {
            words[0] = WIDE;
            words[1] = HIGH;
        }
        RegisterKind::Unknown => return Err(invalid("register types", "untyped register write")),
    }
    Ok(())
}
