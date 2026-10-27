// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use fixedbitset::FixedBitSet;

use super::flow::ControlFlow;
use crate::model::code::CodeItem;

/// The lowest register no live value occupies before instruction `at_index`,
/// other than those in `exclude`.
pub fn find_free_register(code: &CodeItem, at_index: usize, exclude: &[u16]) -> Option<u16> {
    let liveness = RegisterLiveness::new(code);
    let excluded: FixedBitSet = exclude
        .iter()
        .map(|&register| usize::from(register))
        .collect();
    (0..code.registers_size).find(|&register| {
        !liveness.is_live(at_index, register) && !excluded.contains(usize::from(register))
    })
}

/// The instructions whose write of `register` reaches the start of the one at
/// `index`, and whether the value the method was entered with reaches it too.
/// `None` when control flow cannot be followed, which is what an unknown
/// instruction or a branch off an instruction boundary leaves behind.
///
/// An instruction can throw before writing its result, so along an exception
/// edge the register still holds what it held on entry to the throwing
/// instruction. Both words of a wide value are written, so a wide write to the
/// register below this one is a definition of this one.
pub fn reaching_definitions(
    code: &CodeItem,
    index: usize,
    register: u16,
) -> Option<(Vec<usize>, bool)> {
    let count = code.instructions.len();
    if index >= count {
        return None;
    }
    let graph = ControlFlow::new(code)?;
    let mut reachable = FixedBitSet::with_capacity(count);
    let mut pending = vec![0];
    while let Some(current) = pending.pop() {
        if reachable.put(current) {
            continue;
        }
        pending.extend(
            graph
                .successors(current)
                .iter()
                .chain(graph.handlers(current))
                .map(|&next| next as usize),
        );
    }

    let mut visited = FixedBitSet::with_capacity(count);
    let mut definitions = FixedBitSet::with_capacity(count);
    let mut entry = false;
    pending.push(index);
    while let Some(current) = pending.pop() {
        if !reachable.contains(current) || visited.put(current) {
            continue;
        }
        entry |= current == 0;
        for &previous in graph.predecessors(current) {
            let previous = previous as usize;
            if !reachable.contains(previous) {
                continue;
            }
            // Exceptions observe the input even if the normal edge writes a value.
            if graph.handlers(previous).contains(&(current as u32)) {
                pending.push(previous);
            }
            if graph.successors(previous).contains(&(current as u32)) {
                let mut writes = false;
                code.instructions[previous]
                    .visit_written_registers(|reg| writes |= reg == register);
                if writes {
                    definitions.insert(previous);
                } else {
                    pending.push(previous);
                }
            }
        }
    }
    Some((definitions.ones().collect(), entry))
}

/// Register-word liveness at instruction boundaries, including exception edges.
/// Unknown instructions or malformed control flow conservatively keep every register live.
pub struct RegisterLiveness {
    before: Vec<FixedBitSet>,
}

impl RegisterLiveness {
    pub fn new(code: &CodeItem) -> Self {
        let registers = usize::from(code.registers_size);
        let before = Self::analyze(code).unwrap_or_else(|| {
            let mut all = FixedBitSet::with_capacity(registers);
            all.insert_range(..);
            vec![all; code.instructions.len()]
        });
        Self { before }
    }

    pub fn is_live(&self, index: usize, register: u16) -> bool {
        self.before
            .get(index)
            .is_some_and(|live| live.contains(usize::from(register)))
    }

    fn analyze(code: &CodeItem) -> Option<Vec<FixedBitSet>> {
        let graph = ControlFlow::new(code)?;
        let count = code.instructions.len();
        let registers = usize::from(code.registers_size);
        let mut before = vec![FixedBitSet::with_capacity(registers); count];
        let mut live = FixedBitSet::with_capacity(registers);
        let mut pending: std::collections::VecDeque<usize> = (0..count).rev().collect();
        let mut queued = FixedBitSet::with_capacity(count);
        queued.insert_range(..);
        let mark = |live: &mut FixedBitSet, register: u16, value: bool| {
            if usize::from(register) < registers {
                live.set(usize::from(register), value);
            }
        };
        while let Some(index) = pending.pop_front() {
            queued.set(index, false);
            live.clear();
            for &next in graph.successors(index) {
                live.union_with(&before[next as usize]);
            }
            let insn = &code.instructions[index];
            insn.visit_written_registers(|reg| mark(&mut live, reg, false));
            // An instruction can throw before writing its result.
            for &handler in graph.handlers(index) {
                live.union_with(&before[handler as usize]);
            }
            insn.visit_read_registers(|reg| mark(&mut live, reg, true));
            if before[index] != live {
                before[index].clone_from(&live);
                for &previous in graph.predecessors(index) {
                    if !queued.put(previous as usize) {
                        pending.push_back(previous as usize);
                    }
                }
            }
        }
        Some(before)
    }
}

#[cfg(test)]
mod tests;
