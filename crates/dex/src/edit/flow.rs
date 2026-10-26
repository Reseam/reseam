// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

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
        use crate::instruction::Instruction::{
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
