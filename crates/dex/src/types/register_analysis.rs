// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::code::CodeItem;

enum LiveChange {
    Read,
    Write,
}

pub fn find_free_register(code: &CodeItem, at_index: usize, exclude: &[u16]) -> Option<u16> {
    let live = live_registers(code, at_index);
    let excluded = BitSet::from_slice(usize::from(code.registers_size), exclude);

    (0..code.registers_size)
        .find(|&reg| !live.contains(usize::from(reg)) && !excluded.contains(usize::from(reg)))
}

pub fn find_free_registers(
    code: &CodeItem,
    at_index: usize,
    count: usize,
    exclude: &[u16],
) -> Option<Vec<u16>> {
    if count == 0 {
        return Some(Vec::new());
    }

    let live = live_registers(code, at_index);
    let excluded = BitSet::from_slice(usize::from(code.registers_size), exclude);
    let mut free = Vec::with_capacity(count);

    for reg in 0..code.registers_size {
        if live.contains(usize::from(reg)) || excluded.contains(usize::from(reg)) {
            continue;
        }
        free.push(reg);
        if free.len() == count {
            return Some(free);
        }
    }

    None
}

pub fn find_contiguous_free_registers(
    code: &CodeItem,
    at_index: usize,
    count: usize,
    exclude: &[u16],
) -> Option<Vec<u16>> {
    if count == 0 {
        return Some(Vec::new());
    }

    let live = live_registers(code, at_index);
    let excluded = BitSet::from_slice(usize::from(code.registers_size), exclude);
    let mut run_start = None;
    let mut run_len = 0usize;

    for reg in 0..code.registers_size {
        if live.contains(usize::from(reg)) || excluded.contains(usize::from(reg)) {
            run_start = None;
            run_len = 0;
            continue;
        }

        let expected = run_start.unwrap_or(reg) + run_len as u16;
        if run_start.is_none() || reg != expected {
            run_start = Some(reg);
            run_len = 1;
        } else {
            run_len += 1;
        }

        if run_len == count {
            let start = run_start?;
            return Some((start..start + count as u16).collect());
        }
    }

    None
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
    let mut reachable = vec![false; count];
    let mut pending = vec![0];
    while let Some(current) = pending.pop() {
        if std::mem::replace(&mut reachable[current], true) {
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

    let mut visited = vec![false; count];
    let mut definitions = BitSet::new(count);
    let mut entry = false;
    pending.push(index);
    while let Some(current) = pending.pop() {
        if !reachable[current] || std::mem::replace(&mut visited[current], true) {
            continue;
        }
        entry |= current == 0;
        for &previous in graph.predecessors(current) {
            let previous = previous as usize;
            if !reachable[previous] {
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
    Some((
        (0..count).filter(|&i| definitions.contains(i)).collect(),
        entry,
    ))
}

/// Register-word liveness at instruction boundaries, including exception edges.
/// Unknown instructions or malformed control flow conservatively keep every register live.
pub struct RegisterLiveness {
    before: Vec<u64>,
    words: usize,
    register_count: u16,
}

impl RegisterLiveness {
    pub fn new(code: &CodeItem) -> Self {
        let words = usize::from(code.registers_size).div_ceil(64);
        let before = Self::analyze(code, words)
            .unwrap_or_else(|| vec![u64::MAX; code.instructions.len() * words]);
        Self {
            before,
            words,
            register_count: code.registers_size,
        }
    }

    pub fn is_live(&self, index: usize, register: u16) -> bool {
        register < self.register_count
            && self
                .at(index)
                .is_some_and(|live| live[usize::from(register) / 64] & 1 << (register % 64) != 0)
    }

    fn at(&self, index: usize) -> Option<&[u64]> {
        self.before
            .get(index * self.words..(index + 1) * self.words)
    }

    fn analyze(code: &CodeItem, words: usize) -> Option<Vec<u64>> {
        let graph = ControlFlow::new(code)?;
        let count = code.instructions.len();
        let mut before = vec![0u64; count * words];
        let mut live = vec![0u64; words];
        let mut pending: std::collections::VecDeque<usize> = (0..count).rev().collect();
        let mut queued = vec![true; count];
        let set = |live: &mut [u64], reg: u16, change: LiveChange| {
            let (word, bit) = (usize::from(reg) / 64, 1u64 << (reg % 64));
            if let Some(word) = live.get_mut(word) {
                *word = match change {
                    LiveChange::Read => *word | bit,
                    LiveChange::Write => *word & !bit,
                };
            }
        };
        let union = |live: &mut [u64], before: &[u64], index: usize| {
            for (word, other) in live.iter_mut().zip(&before[index * words..]) {
                *word |= other;
            }
        };
        while let Some(index) = pending.pop_front() {
            queued[index] = false;
            live.fill(0);
            for &next in graph.successors(index) {
                union(&mut live, &before, next as usize);
            }
            let insn = &code.instructions[index];
            insn.visit_written_registers(|reg| set(&mut live, reg, LiveChange::Write));
            // An instruction can throw before writing its result.
            for &handler in graph.handlers(index) {
                union(&mut live, &before, handler as usize);
            }
            insn.visit_read_registers(|reg| set(&mut live, reg, LiveChange::Read));
            let slot = &mut before[index * words..(index + 1) * words];
            if slot != live.as_slice() {
                slot.copy_from_slice(&live);
                for &previous in graph.predecessors(index) {
                    if !std::mem::replace(&mut queued[previous as usize], true) {
                        pending.push_back(previous as usize);
                    }
                }
            }
        }
        Some(before)
    }
}

struct Edges {
    start: Vec<u32>,
    targets: Vec<u32>,
}

impl Edges {
    fn of(&self, index: usize) -> &[u32] {
        &self.targets[self.start[index] as usize..self.start[index + 1] as usize]
    }
}

pub(crate) struct ControlFlow {
    successors: Edges,
    predecessors: Edges,
    handler_of: Vec<u32>,
    handler_lists: Vec<Vec<u32>>,
}

impl ControlFlow {
    pub fn new(code: &CodeItem) -> Option<Self> {
        use super::instruction::Instruction::{
            FillArrayDataPayload, Goto, Goto16, Goto32, IfEq, IfEqz, IfGe, IfGez, IfGt, IfGtz,
            IfLe, IfLez, IfLt, IfLtz, IfNe, IfNez, PackedSwitch, PackedSwitchPayload, Raw, Return,
            ReturnObject, ReturnVoid, ReturnWide, SparseSwitch, SparseSwitchPayload, Throw,
        };
        let count = code.instructions.len();
        let offsets: Vec<u32> = code
            .instructions
            .iter()
            .scan(0u32, |offset, insn| {
                let current = *offset;
                *offset += insn.code_units();
                Some(current)
            })
            .collect();
        let target = |index: usize, delta: i32| -> Option<u32> {
            let address = i64::from(offsets[index]) + i64::from(delta);
            offsets
                .binary_search(&u32::try_from(address).ok()?)
                .ok()
                .map(|i| i as u32)
        };
        let mut successors = Edges {
            start: Vec::with_capacity(count + 1),
            targets: Vec::with_capacity(count + count / 4),
        };
        successors.start.push(0);
        for (index, insn) in code.instructions.iter().enumerate() {
            let next = &mut successors.targets;
            match insn {
                Raw { .. } => return None,
                ReturnVoid
                | Return { .. }
                | ReturnWide { .. }
                | ReturnObject { .. }
                | Throw { .. }
                | PackedSwitchPayload(_)
                | SparseSwitchPayload(_)
                | FillArrayDataPayload(_) => {}
                Goto { offset } => next.push(target(index, i32::from(*offset))?),
                Goto16 { offset } => next.push(target(index, i32::from(*offset))?),
                Goto32 { offset } => next.push(target(index, *offset)?),
                _ => {
                    if index + 1 < count {
                        next.push(index as u32 + 1);
                    }
                    match insn {
                        IfEq { offset, .. }
                        | IfNe { offset, .. }
                        | IfLt { offset, .. }
                        | IfGe { offset, .. }
                        | IfGt { offset, .. }
                        | IfLe { offset, .. }
                        | IfEqz { offset, .. }
                        | IfNez { offset, .. }
                        | IfLtz { offset, .. }
                        | IfGez { offset, .. }
                        | IfGtz { offset, .. }
                        | IfLez { offset, .. } => {
                            next.push(target(index, i32::from(*offset))?);
                        }
                        PackedSwitch { payload_offset, .. } => {
                            let PackedSwitchPayload(payload) =
                                &code.instructions[target(index, *payload_offset)? as usize]
                            else {
                                return None;
                            };
                            for offset in &payload.targets {
                                next.push(target(index, *offset)?);
                            }
                        }
                        SparseSwitch { payload_offset, .. } => {
                            let SparseSwitchPayload(payload) =
                                &code.instructions[target(index, *payload_offset)? as usize]
                            else {
                                return None;
                            };
                            for (_, offset) in &payload.keys_and_targets {
                                next.push(target(index, *offset)?);
                            }
                        }
                        _ => {}
                    }
                }
            }
            successors.start.push(next.len() as u32);
        }

        let (handler_of, handler_lists) = Self::exception_edges(code, &offsets)?;
        let predecessors = Self::reverse_edges(&successors, &handler_of, &handler_lists);
        Some(Self {
            successors,
            predecessors,
            handler_of,
            handler_lists,
        })
    }

    fn reverse_edges(successors: &Edges, handler_of: &[u32], handler_lists: &[Vec<u32>]) -> Edges {
        let count = handler_of.len();
        let handlers = |index: usize| -> &[u32] {
            handler_lists
                .get(handler_of[index] as usize)
                .map_or(&[], Vec::as_slice)
        };
        let mut start = vec![0u32; count + 1];
        for index in 0..count {
            for &next in successors.of(index).iter().chain(handlers(index)) {
                start[next as usize + 1] += 1;
            }
        }
        for index in 0..count {
            start[index + 1] += start[index];
        }
        let mut fill = start.clone();
        let mut targets = vec![0u32; start[count] as usize];
        for index in 0..count {
            for &next in successors.of(index).iter().chain(handlers(index)) {
                targets[fill[next as usize] as usize] = index as u32;
                fill[next as usize] += 1;
            }
        }
        Edges { start, targets }
    }

    fn exception_edges(code: &CodeItem, offsets: &[u32]) -> Option<(Vec<u32>, Vec<Vec<u32>>)> {
        let count = code.instructions.len();
        let mut handler_of = vec![u32::MAX; count];
        let mut handler_lists = Vec::with_capacity(code.tries.len());
        for protected in &code.tries {
            let end = protected
                .start_addr
                .checked_add(u32::from(protected.insn_count))?;
            let handler = code.catch_handlers.get(protected.handler_idx)?;
            let targets: Vec<u32> = handler
                .typed_catches
                .iter()
                .map(|catch| catch.addr)
                .chain(handler.catch_all_addr)
                .map(|addr| offsets.binary_search(&addr).ok().map(|i| i as u32))
                .collect::<Option<_>>()?;
            let first = offsets.partition_point(|&offset| offset < protected.start_addr);
            let last = offsets.partition_point(|&offset| offset < end);
            handler_of[first..last].fill(handler_lists.len() as u32);
            handler_lists.push(targets);
        }

        Some((handler_of, handler_lists))
    }

    pub fn successors(&self, index: usize) -> &[u32] {
        self.successors.of(index)
    }

    /// Where control goes when the instruction throws.
    pub fn handlers(&self, index: usize) -> &[u32] {
        self.handler_lists
            .get(self.handler_of[index] as usize)
            .map_or(&[], Vec::as_slice)
    }

    pub fn predecessors(&self, index: usize) -> &[u32] {
        self.predecessors.of(index)
    }
}

fn live_registers(code: &CodeItem, at_index: usize) -> BitSet {
    let liveness = RegisterLiveness::new(code);
    BitSet {
        words: liveness
            .at(at_index)
            .map_or_else(|| vec![0; liveness.words], <[u64]>::to_vec),
    }
}

#[derive(Clone, PartialEq, Eq)]
struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    fn new(capacity: usize) -> Self {
        Self {
            words: vec![0; capacity.div_ceil(64)],
        }
    }

    fn from_slice(capacity: usize, members: &[u16]) -> Self {
        let mut set = Self::new(capacity);
        for &member in members {
            set.insert(usize::from(member));
        }
        set
    }

    fn insert(&mut self, member: usize) {
        if let Some(word) = self.words.get_mut(member / 64) {
            *word |= 1 << (member % 64);
        }
    }

    fn contains(&self, member: usize) -> bool {
        self.words
            .get(member / 64)
            .is_some_and(|word| word & (1 << (member % 64)) != 0)
    }
}

#[cfg(test)]
mod tests;
