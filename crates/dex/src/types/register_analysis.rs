// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::code::CodeItem;

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
    let ControlFlow {
        successors,
        handlers,
        ..
    } = ControlFlow::new(code)?;
    // One bit per instruction that could have defined the register, plus one
    // for the value the method was called with.
    let entry = count;
    let mut before = vec![BitSet::new(count + 1); count];
    before[0].insert(entry);
    let mut pending = std::collections::VecDeque::from([0usize]);
    let mut queued = vec![false; count];
    queued[0] = true;
    while let Some(index) = pending.pop_front() {
        queued[index] = false;
        let incoming = before[index].clone();
        let mut writes = false;
        code.instructions[index].visit_written_registers(|reg| writes |= reg == register);
        let outgoing = if writes {
            let mut defined = BitSet::new(count + 1);
            defined.insert(index);
            defined
        } else {
            incoming.clone()
        };
        let mut propagate = |edges: &[usize], set: &BitSet, before: &mut Vec<BitSet>| {
            for &next in edges {
                if before[next].merge(set) && !queued[next] {
                    queued[next] = true;
                    pending.push_back(next);
                }
            }
        };
        propagate(&successors[index], &outgoing, &mut before);
        propagate(&handlers[index], &incoming, &mut before);
    }
    let reaching = &before[index];
    Some((
        (0..count).filter(|&i| reaching.contains(i)).collect(),
        reaching.contains(entry),
    ))
}

/// Register-word liveness at instruction boundaries, including exception edges.
/// Unknown instructions or malformed control flow conservatively keep every register live.
pub struct RegisterLiveness {
    before: Vec<BitSet>,
    register_count: u16,
}

impl RegisterLiveness {
    pub fn new(code: &CodeItem) -> Self {
        let count = code.instructions.len();
        let before = Self::analyze(code)
            .unwrap_or_else(|| vec![BitSet::full(usize::from(code.registers_size)); count]);
        Self {
            before,
            register_count: code.registers_size,
        }
    }

    pub fn is_live(&self, index: usize, register: u16) -> bool {
        self.before
            .get(index)
            .is_some_and(|live| live.contains(usize::from(register)))
    }

    fn analyze(code: &CodeItem) -> Option<Vec<BitSet>> {
        let ControlFlow {
            successors,
            handlers,
            predecessors,
        } = ControlFlow::new(code)?;
        let count = code.instructions.len();
        let mut before = vec![BitSet::new(usize::from(code.registers_size)); count];
        let mut pending: std::collections::VecDeque<usize> = (0..count).rev().collect();
        let mut queued = vec![true; count];
        while let Some(index) = pending.pop_front() {
            queued[index] = false;
            let mut live = BitSet::new(usize::from(code.registers_size));
            for &next in &successors[index] {
                live.union(&before[next]);
            }
            code.instructions[index].visit_written_registers(|reg| live.remove(usize::from(reg)));
            // An instruction can throw before writing its result.
            for &handler in &handlers[index] {
                live.union(&before[handler]);
            }
            code.instructions[index].visit_read_registers(|reg| live.insert(usize::from(reg)));
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

fn live_registers(code: &CodeItem, at_index: usize) -> BitSet {
    let mut liveness = RegisterLiveness::new(code);
    if at_index < liveness.before.len() {
        liveness.before.swap_remove(at_index)
    } else {
        BitSet::new(usize::from(liveness.register_count))
    }
}

/// A set of registers, or of the instruction indices that defined one.
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

    fn full(capacity: usize) -> Self {
        Self {
            words: vec![u64::MAX; capacity.div_ceil(64)],
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

    fn remove(&mut self, member: usize) {
        if let Some(word) = self.words.get_mut(member / 64) {
            *word &= !(1 << (member % 64));
        }
    }

    fn union(&mut self, other: &Self) {
        for (word, other) in self.words.iter_mut().zip(&other.words) {
            *word |= other;
        }
    }

    /// Unions `other` in and reports whether that added anything.
    fn merge(&mut self, other: &Self) -> bool {
        let mut changed = false;
        for (word, other) in self.words.iter_mut().zip(&other.words) {
            let merged = *word | other;
            changed |= merged != *word;
            *word = merged;
        }
        changed
    }

    fn contains(&self, member: usize) -> bool {
        self.words
            .get(member / 64)
            .is_some_and(|word| word & (1 << (member % 64)) != 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::types::code::CodeItem;
    use crate::types::instruction::Instruction;

    use super::{
        find_contiguous_free_registers, find_free_register, find_free_registers,
        reaching_definitions,
    };

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

    /// Both words of a wide write are definitions, so the pair below a
    /// register redefines it.
    #[test]
    fn a_wide_write_to_the_register_below_defines_this_one() {
        let code = code(
            vec![
                Instruction::Const { dest: 1, value: 1 },
                Instruction::ConstWide16 { dest: 0, value: 7 },
                Instruction::Return { src: 1 },
            ],
            3,
        );
        assert_eq!(reaching_definitions(&code, 2, 1), Some((vec![1], false)));
        assert_eq!(reaching_definitions(&code, 0, 1), Some((vec![], true)));
    }

    /// An instruction inside a try can throw before it writes, so its handler
    /// sees what reached the instruction, not what it would have written.
    #[test]
    fn an_exception_edge_carries_the_definitions_reaching_the_thrower() {
        let mut code = code(
            vec![
                Instruction::Const4 { dest: 1, value: 1 },
                Instruction::IgetWide {
                    dest: 1,
                    obj: 0,
                    field: crate::FieldIdx(0),
                },
                Instruction::ReturnWide { src: 1 },
                Instruction::MoveException { dest: 0 },
                Instruction::Return { src: 1 },
            ],
            3,
        );
        code.tries.push(crate::TryItem {
            start_addr: 1,
            insn_count: 2,
            handler_idx: 0,
        });
        code.catch_handlers.push(crate::CatchHandler {
            typed_catches: vec![],
            catch_all_addr: Some(4),
        });
        assert_eq!(reaching_definitions(&code, 2, 1), Some((vec![1], false)));
        assert_eq!(reaching_definitions(&code, 4, 1), Some((vec![0], false)));
    }
}
