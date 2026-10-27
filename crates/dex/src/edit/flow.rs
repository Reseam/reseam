// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::instruction::Instruction;
use crate::instruction::branch::Addresses;
use crate::model::code::CodeItem;

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
        let count = code.instructions.len();
        let addresses = Addresses::of(&code.instructions);
        let mut successors = Edges {
            start: Vec::with_capacity(count + 1),
            targets: Vec::with_capacity(count + count / 4),
        };
        successors.start.push(0);
        for index in 0..count {
            normal_successors(
                &code.instructions,
                &addresses,
                index,
                &mut successors.targets,
            )?;
            successors.start.push(successors.targets.len() as u32);
        }
        let (handler_of, handler_lists) = Self::exception_edges(code, &addresses)?;
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

    fn exception_edges(
        code: &CodeItem,
        addresses: &Addresses,
    ) -> Option<(Vec<u32>, Vec<Vec<u32>>)> {
        let count = code.instructions.len();
        let starts = &addresses[..count];
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
                .map(|addr| {
                    addresses
                        .index_of(addr)
                        .filter(|&i| i < count)
                        .map(|i| i as u32)
                })
                .collect::<Option<_>>()?;
            let first = starts.partition_point(|&offset| offset < protected.start_addr);
            let last = starts.partition_point(|&offset| offset < end);
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

/// Where control goes after instruction `index` completes normally; `None`
/// when that cannot be followed: an unknown instruction, or a branch that does
/// not land on an instruction.
fn normal_successors(
    instructions: &[Instruction],
    addresses: &Addresses,
    index: usize,
    out: &mut Vec<u32>,
) -> Option<()> {
    let instruction = &instructions[index];
    if matches!(instruction, Instruction::Raw { .. }) {
        return None;
    }
    let target = |delta| {
        addresses
            .target(index, delta)
            .filter(|&target| target < instructions.len())
    };
    if instruction.falls_through() && index + 1 < instructions.len() {
        out.push(index as u32 + 1);
    }
    if let Some(delta) = instruction.branch_offset() {
        out.push(target(delta)? as u32);
    }
    if instruction.is_switch() {
        let payload = &instructions[target(instruction.payload_offset()?)?];
        for delta in payload.switch_targets()? {
            out.push(target(delta)? as u32);
        }
    }
    Some(())
}
