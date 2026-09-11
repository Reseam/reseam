// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::code::CodeItem;

pub fn find_free_register(code: &CodeItem, at_index: usize, exclude: &[u16]) -> Option<u16> {
    let live = live_registers(code, at_index);
    let excluded = RegisterSet::from_slice(code.registers_size, exclude);

    (0..code.registers_size).find(|&reg| !live.contains(reg) && !excluded.contains(reg))
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
    let excluded = RegisterSet::from_slice(code.registers_size, exclude);
    let mut free = Vec::with_capacity(count);

    for reg in 0..code.registers_size {
        if live.contains(reg) || excluded.contains(reg) {
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
    let excluded = RegisterSet::from_slice(code.registers_size, exclude);
    let mut run_start = None;
    let mut run_len = 0usize;

    for reg in 0..code.registers_size {
        if live.contains(reg) || excluded.contains(reg) {
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

/// Register-word liveness at instruction boundaries, including exception edges.
/// Unknown instructions or malformed control flow conservatively keep every register live.
pub struct RegisterLiveness {
    before: Vec<RegisterSet>,
    register_count: u16,
}

impl RegisterLiveness {
    pub fn new(code: &CodeItem) -> Self {
        let count = code.instructions.len();
        let before = Self::analyze(code)
            .unwrap_or_else(|| vec![RegisterSet::full(code.registers_size); count]);
        Self {
            before,
            register_count: code.registers_size,
        }
    }

    pub fn is_live(&self, index: usize, register: u16) -> bool {
        self.before
            .get(index)
            .is_some_and(|live| live.contains(register))
    }

    fn analyze(code: &CodeItem) -> Option<Vec<RegisterSet>> {
        let ControlFlow {
            successors,
            handlers,
            predecessors,
        } = ControlFlow::new(code)?;
        let count = code.instructions.len();
        let mut before = vec![RegisterSet::new(code.registers_size); count];
        let mut pending: std::collections::VecDeque<usize> = (0..count).rev().collect();
        let mut queued = vec![true; count];
        while let Some(index) = pending.pop_front() {
            queued[index] = false;
            let mut live = RegisterSet::new(code.registers_size);
            for &next in &successors[index] {
                live.union(&before[next]);
            }
            code.instructions[index].visit_written_registers(|reg| live.remove(reg));
            // An instruction can throw before writing its result.
            for &handler in &handlers[index] {
                live.union(&before[handler]);
            }
            code.instructions[index].visit_read_registers(|reg| live.insert(reg));
            if before[index] != live {
                before[index] = live;
                for &previous in &predecessors[index] {
                    if !queued[previous] {
                        pending.push_back(previous);
                        queued[previous] = true;
                    }
                }
            }
        }
        Some(before)
    }
}

pub(crate) struct ControlFlow {
    pub successors: Vec<Vec<usize>>,
    pub handlers: Vec<Vec<usize>>,
    pub predecessors: Vec<Vec<usize>>,
}

impl ControlFlow {
    pub fn new(code: &CodeItem) -> Option<Self> {
        use super::instruction::Instruction::*;
        let count = code.instructions.len();
        let offsets: Vec<u32> = code
            .instructions
            .iter()
            .scan(0u32, |offset, insn| {
                let current = *offset;
                *offset += insn.code_units() as u32;
                Some(current)
            })
            .collect();
        let target = |index: usize, delta: i32| -> Option<usize> {
            let address = i64::from(offsets[index]) + i64::from(delta);
            offsets.binary_search(&u32::try_from(address).ok()?).ok()
        };
        let mut successors = vec![Vec::new(); count];
        let mut handlers = vec![Vec::new(); count];
        let mut predecessors = vec![Vec::new(); count];
        for (index, insn) in code.instructions.iter().enumerate() {
            let next = &mut successors[index];
            match insn {
                RawInstruction { .. } => return None,
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
                        next.push(index + 1);
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
                                &code.instructions[target(index, *payload_offset)?]
                            else {
                                return None;
                            };
                            for offset in &payload.targets {
                                next.push(target(index, *offset)?);
                            }
                        }
                        SparseSwitch { payload_offset, .. } => {
                            let SparseSwitchPayload(payload) =
                                &code.instructions[target(index, *payload_offset)?]
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
        }
        for protected in &code.tries {
            let end = protected
                .start_addr
                .checked_add(u32::from(protected.insn_count))?;
            let handler = code.catch_handlers.get(protected.handler_idx)?;
            let targets: Vec<usize> = handler
                .typed_catches
                .iter()
                .map(|catch| catch.addr)
                .chain(handler.catch_all_addr)
                .map(|addr| offsets.binary_search(&addr).ok())
                .collect::<Option<_>>()?;
            for (index, &offset) in offsets.iter().enumerate() {
                if protected.start_addr <= offset && offset < end {
                    handlers[index].extend_from_slice(&targets);
                }
            }
        }
        for index in 0..count {
            for &next in successors[index].iter().chain(&handlers[index]) {
                predecessors[next].push(index);
            }
        }
        Some(Self {
            successors,
            handlers,
            predecessors,
        })
    }
}

fn live_registers(code: &CodeItem, at_index: usize) -> RegisterSet {
    let mut liveness = RegisterLiveness::new(code);
    if at_index < liveness.before.len() {
        liveness.before.swap_remove(at_index)
    } else {
        RegisterSet::new(liveness.register_count)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct RegisterSet {
    words: Vec<u64>,
}

impl RegisterSet {
    fn new(register_count: u16) -> Self {
        Self {
            words: vec![0; usize::from(register_count).div_ceil(64)],
        }
    }

    fn full(register_count: u16) -> Self {
        Self {
            words: vec![u64::MAX; usize::from(register_count).div_ceil(64)],
        }
    }

    fn from_slice(register_count: u16, registers: &[u16]) -> Self {
        let mut set = Self::new(register_count);
        for &register in registers {
            set.insert(register);
        }
        set
    }

    fn insert(&mut self, register: u16) {
        if let Some(word) = self.words.get_mut(usize::from(register) / 64) {
            *word |= 1 << (register % 64);
        }
    }

    fn remove(&mut self, register: u16) {
        if let Some(word) = self.words.get_mut(usize::from(register) / 64) {
            *word &= !(1 << (register % 64));
        }
    }

    fn union(&mut self, other: &Self) {
        for (word, other) in self.words.iter_mut().zip(&other.words) {
            *word |= other;
        }
    }

    fn contains(&self, register: u16) -> bool {
        self.words
            .get(usize::from(register) / 64)
            .is_some_and(|word| word & (1 << (register % 64)) != 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::types::code::CodeItem;
    use crate::types::instruction::Instruction;

    use super::{find_contiguous_free_registers, find_free_register, find_free_registers};

    fn code(instructions: Vec<Instruction>, registers_size: u16) -> CodeItem {
        CodeItem {
            registers_size,
            ins_size: 0,
            outs_size: 0,
            debug_info: None,
            instructions,
            tries: Vec::new(),
            catch_handlers: Vec::new(),
        }
    }

    #[test]
    fn finds_first_available_register() {
        let code = code(
            vec![
                Instruction::Const { dest: 0, value: 1 },
                Instruction::AddInt {
                    dest: 1,
                    a: 0,
                    b: 2,
                },
                Instruction::Return { src: 1 },
            ],
            5,
        );

        assert_eq!(find_free_register(&code, 1, &[]), Some(1));
    }

    #[test]
    fn respects_excluded_registers() {
        let code = code(
            vec![
                Instruction::Const { dest: 0, value: 1 },
                Instruction::Return { src: 0 },
            ],
            4,
        );

        assert_eq!(find_free_registers(&code, 1, 2, &[1]), Some(vec![2, 3]));
    }

    #[test]
    fn finds_contiguous_range_after_invoke_range() {
        let code = code(
            vec![
                Instruction::InvokeStaticRange {
                    method: crate::types::MethodIdx(0),
                    first_reg: 1,
                    count: 2,
                },
                Instruction::ReturnVoid,
            ],
            6,
        );

        assert_eq!(
            find_contiguous_free_registers(&code, 0, 2, &[]),
            Some(vec![3, 4])
        );
    }
    #[test]
    fn keeps_both_words_of_incoming_wide_values_live() {
        let mut code = code(
            vec![
                Instruction::IputBoolean {
                    src: 0,
                    obj: 1,
                    field: crate::FieldIdx(0),
                },
                Instruction::IputWide {
                    src: 2,
                    obj: 1,
                    field: crate::FieldIdx(1),
                },
                Instruction::ReturnVoid,
            ],
            4,
        );
        code.ins_size = 4;
        assert_eq!(find_free_register(&code, 0, &[]), None);
        assert_eq!(find_free_registers(&code, 0, 1, &[]), None);
        assert_eq!(find_contiguous_free_registers(&code, 0, 1, &[]), None);
        assert_eq!(find_free_register(&code, 1, &[]), Some(0));
    }

    #[test]
    fn follows_branches_and_back_edges_before_reusing_a_register() {
        let code = code(
            vec![
                Instruction::IfEqz { a: 0, offset: 4 },
                Instruction::Const16 { dest: 1, value: 0 },
                Instruction::Return { src: 1 },
            ],
            2,
        );
        assert_eq!(find_free_register(&code, 0, &[]), None);
        assert_eq!(find_free_register(&code, 1, &[]), Some(0));
        let code = super::tests::code(
            vec![
                Instruction::SputWide {
                    src: 1,
                    field: crate::FieldIdx(0),
                },
                Instruction::IfNez { a: 0, offset: -2 },
                Instruction::ReturnVoid,
            ],
            3,
        );
        assert_eq!(find_free_register(&code, 1, &[]), None);
    }

    #[test]
    fn preserves_values_read_by_exception_handlers_before_a_write_completes() {
        let mut code = code(
            vec![
                Instruction::IgetWide {
                    dest: 1,
                    obj: 0,
                    field: crate::FieldIdx(0),
                },
                Instruction::ReturnWide { src: 1 },
                Instruction::MoveException { dest: 0 },
                Instruction::ReturnWide { src: 1 },
            ],
            3,
        );
        code.tries.push(crate::TryItem {
            start_addr: 0,
            insn_count: 2,
            handler_idx: 0,
        });
        code.catch_handlers.push(crate::CatchHandler {
            typed_catches: vec![],
            catch_all_addr: Some(3),
        });
        assert_eq!(find_free_register(&code, 0, &[]), None);
    }
}
