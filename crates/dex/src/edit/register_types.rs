// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::flow::ControlFlow;
use crate::error::{Result, invalid};
use crate::instruction::Instruction;
use crate::instruction::operands::{Access, RegisterKind};
use crate::model::code::CodeItem;

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
    /// The type bits of each queried `(instruction, register)` on entry to
    /// that instruction, joined over every path that reaches it.
    pub fn new(
        code: &CodeItem,
        incoming: &[RegisterKind],
        queries: &[(usize, u16)],
    ) -> Result<Self> {
        let graph = ControlFlow::new(code)
            .ok_or_else(|| invalid("register types", "invalid control flow"))?;
        let mut queried = queries
            .iter()
            .map(|&site| (site, UNDEFINED))
            .collect::<rustc_hash::FxHashMap<_, _>>();
        if code.instructions.is_empty() {
            return Ok(Self { queried });
        }
        let blocks = Blocks::new(&graph, code.instructions.len());
        let before = blocks.fixpoint(code, &graph, entry_frame(code, incoming)?)?;
        let mut sites = queries.to_vec();
        sites.sort_unstable();
        for (block, state) in before.into_iter().enumerate() {
            if let Some(state) = state {
                blocks.answer(code, block, state, &sites, &mut queried)?;
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

/// The frame on entry: undefined locals, then the incoming arguments.
fn entry_frame(code: &CodeItem, incoming: &[RegisterKind]) -> Result<Vec<u8>> {
    let base = code
        .registers_size
        .checked_sub(code.ins_size)
        .ok_or_else(|| invalid("register types", "parameters exceed frame"))?;
    let mut entry = vec![UNDEFINED; usize::from(code.registers_size)];
    let mut register = usize::from(base);
    for &kind in incoming {
        write(&mut entry, register, kind)?;
        register += usize::from(kind.words());
    }
    if register != entry.len() {
        return Err(invalid(
            "register types",
            "parameter width does not match ins_size",
        ));
    }
    Ok(entry)
}

/// Basic blocks: where each starts, then the end of the body, and the block
/// of each instruction.
struct Blocks {
    starts: Vec<usize>,
    block_of: Vec<usize>,
}

impl Blocks {
    fn new(graph: &ControlFlow, count: usize) -> Self {
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
        Self { starts, block_of }
    }

    /// The frame on entry to each block, `None` for unreachable ones.
    fn fixpoint(
        &self,
        code: &CodeItem,
        graph: &ControlFlow,
        entry: Vec<u8>,
    ) -> Result<Vec<Option<Vec<u8>>>> {
        let mut before = vec![None; self.starts.len() - 1];
        before[0] = Some(entry);
        let mut queue = std::collections::VecDeque::from([0]);
        let mut queued = vec![false; before.len()];
        queued[0] = true;
        while let Some(block) = queue.pop_front() {
            queued[block] = false;
            let mut state = before[block]
                .clone()
                .expect("queued blocks have an entry frame");
            let end = self.starts[block + 1];
            for index in self.starts[block]..end {
                for &handler in graph.handlers(index) {
                    let target = self.block_of[handler as usize];
                    propagate(target, &state, &mut before, &mut queue, &mut queued);
                }
                transfer(&code.instructions[index], &mut state)?;
            }
            for &next in graph.successors(end - 1) {
                let target = self.block_of[next as usize];
                propagate(target, &state, &mut before, &mut queue, &mut queued);
            }
        }
        Ok(before)
    }

    /// Records the queried registers of `block`, stepping its frame forward
    /// to each queried instruction.
    fn answer(
        &self,
        code: &CodeItem,
        block: usize,
        mut state: Vec<u8>,
        sites: &[(usize, u16)],
        queried: &mut rustc_hash::FxHashMap<(usize, u16), u8>,
    ) -> Result<()> {
        let (start, end) = (self.starts[block], self.starts[block + 1]);
        let first = sites.partition_point(|&(index, _)| index < start);
        let last = sites.partition_point(|&(index, _)| index < end);
        let mut requested = sites[first..last].iter().peekable();
        for index in start..end {
            while let Some(&(site, register)) = requested.next_if(|&&(site, _)| site == index) {
                queried.insert(
                    (site, register),
                    state
                        .get(usize::from(register))
                        .copied()
                        .unwrap_or(UNDEFINED),
                );
            }
            if requested.peek().is_none() {
                break;
            }
            transfer(&code.instructions[index], &mut state)?;
        }
        Ok(())
    }
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
            result = operand
                .kind
                .ok_or_else(|| invalid("register types", "untyped register write"))
                .and_then(|kind| write(state, operand.register as usize, kind));
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
    }
    Ok(())
}
