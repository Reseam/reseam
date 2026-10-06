// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::code::CodeItem;
use super::instruction::Instruction;
use super::register_analysis::ControlFlow;
use super::register_operands::{Access, RegisterKind};
use crate::error::{Result, invalid};

const UNDEFINED: u8 = 1;
const VALUE: u8 = 2;
const OBJECT: u8 = 4;
const ZERO: u8 = 8;
const WIDE: u8 = 16;
const HIGH: u8 = 32;

pub(crate) struct RegisterTypes {
    queried: rustc_hash::FxHashMap<(usize, u16), u8>,
}

impl RegisterTypes {
    pub fn new(
        code: &CodeItem,
        incoming: &[RegisterKind],
        queries: &[(usize, u16)],
    ) -> Result<Self> {
        let graph = ControlFlow::new(code)
            .ok_or_else(|| invalid("register types", "invalid control flow"))?;
        let count = code.instructions.len();
        let mut queried = queries
            .iter()
            .map(|&site| (site, UNDEFINED))
            .collect::<rustc_hash::FxHashMap<_, _>>();
        if count == 0 {
            return Ok(Self { queried });
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
        let (starts, block_of) = block_layout(&graph, count);
        let mut before = vec![None; starts.len() - 1];
        before[0] = Some(entry);
        let mut queue = std::collections::VecDeque::from([0]);
        let mut queued = vec![false; before.len()];
        queued[0] = true;
        while let Some(block) = queue.pop_front() {
            queued[block] = false;
            let mut state = before[block]
                .as_ref()
                .expect("queued blocks have an entry frame")
                .clone();
            for index in starts[block]..starts[block + 1] {
                for &handler in graph.handlers(index) {
                    propagate(
                        block_of[handler as usize],
                        &state,
                        &mut before,
                        &mut queue,
                        &mut queued,
                    );
                }
                transfer(&code.instructions[index], &mut state)?;
                if index + 1 == starts[block + 1] {
                    for &next in graph.successors(index) {
                        propagate(
                            block_of[next as usize],
                            &state,
                            &mut before,
                            &mut queue,
                            &mut queued,
                        );
                    }
                }
            }
        }
        let mut sites = queries.to_vec();
        sites.sort_unstable();
        for (block, bounds) in starts.windows(2).enumerate() {
            let Some(mut state) = before[block].take() else {
                continue;
            };
            let first = sites.partition_point(|&(index, _)| index < bounds[0]);
            let last = sites.partition_point(|&(index, _)| index < bounds[1]);
            let mut requested = sites[first..last].iter().peekable();
            if requested.peek().is_none() {
                continue;
            }
            for (index, instruction) in code
                .instructions
                .iter()
                .enumerate()
                .take(bounds[1])
                .skip(bounds[0])
            {
                while let Some(&&(site, register)) = requested.peek() {
                    if site != index {
                        break;
                    }
                    queried.insert(
                        (site, register),
                        state.get(register as usize).copied().unwrap_or(UNDEFINED),
                    );
                    requested.next();
                }
                if requested.peek().is_none() {
                    break;
                }
                transfer(instruction, &mut state)?;
            }
        }
        Ok(Self { queried })
    }

    pub fn kind(&self, index: usize, register: u16) -> Result<RegisterKind> {
        let bits = self
            .queried
            .get(&(index, register))
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

fn block_layout(graph: &ControlFlow, count: usize) -> (Vec<usize>, Vec<usize>) {
    let mut leaders = vec![false; count];
    leaders[0] = true;
    for index in 0..count {
        let normal = graph.successors(index);
        if normal != [index as u32 + 1] {
            for &target in normal {
                leaders[target as usize] = true;
            }
            if index + 1 < count {
                leaders[index + 1] = true;
            }
        }
        for &handler in graph.handlers(index) {
            leaders[handler as usize] = true;
        }
    }
    let mut starts: Vec<_> = leaders
        .iter()
        .enumerate()
        .filter_map(|(index, &leader)| leader.then_some(index))
        .collect();
    starts.push(count);
    let mut block_of = vec![0; count];
    for (block, bounds) in starts.windows(2).enumerate() {
        block_of[bounds[0]..bounds[1]].fill(block);
    }
    (starts, block_of)
}

fn propagate(
    block: usize,
    state: &[u8],
    before: &mut [Option<Vec<u8>>],
    queue: &mut std::collections::VecDeque<usize>,
    queued: &mut [bool],
) {
    let changed = if let Some(previous) = &mut before[block] {
        let mut changed = false;
        for (previous, &value) in previous.iter_mut().zip(state) {
            let combined = *previous | value;
            changed |= combined != *previous;
            *previous = combined;
        }
        changed
    } else {
        before[block] = Some(state.to_vec());
        true
    };
    if changed && !std::mem::replace(&mut queued[block], true) {
        queue.push_back(block);
    }
}

fn transfer(instruction: &Instruction, state: &mut [u8]) -> Result<()> {
    let source = match instruction {
        Instruction::Move { src, .. } | Instruction::MoveObject { src, .. } => {
            Some(u16::from(*src))
        }
        Instruction::MoveFrom16 { src, .. }
        | Instruction::MoveObjectFrom16 { src, .. }
        | Instruction::Move16 { src, .. }
        | Instruction::MoveObject16 { src, .. } => Some(*src),
        _ => None,
    };
    let source = source
        .map(|register| {
            state
                .get(register as usize)
                .copied()
                .ok_or_else(|| invalid("register types", "move source outside frame"))
        })
        .transpose()?;
    let zero = matches!(
        instruction,
        Instruction::Const4 { value: 0, .. }
            | Instruction::Const16 { value: 0, .. }
            | Instruction::Const { value: 0, .. }
            | Instruction::ConstHigh16 { value: 0, .. }
    );
    let mut result = Ok(());
    instruction.visit_operands(|operand| {
        if operand.access != Access::Read && result.is_ok() {
            result = write(state, operand.register as usize, operand.kind);
            if result.is_ok() {
                if let Some(source) = source {
                    state[operand.register as usize] = source;
                } else if zero {
                    state[operand.register as usize] = ZERO;
                }
            }
        }
    });
    result
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
