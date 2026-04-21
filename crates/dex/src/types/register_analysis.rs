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

fn live_registers(code: &CodeItem, at_index: usize) -> RegisterSet {
    let mut live = RegisterSet::new(code.registers_size);
    let len = code.instructions.len();
    if at_index >= len {
        return live;
    }

    for insn in code.instructions[..at_index].iter().rev() {
        insn.visit_written_registers(|register| live.insert(register));
    }

    let mut written_forward = RegisterSet::new(code.registers_size);
    for insn in &code.instructions[at_index..] {
        insn.visit_read_registers(|register| {
            if !written_forward.contains(register) {
                live.insert(register);
            }
        });
        insn.visit_written_registers(|register| written_forward.insert(register));
    }

    live
}

struct RegisterSet {
    bits: Vec<bool>,
}

impl RegisterSet {
    fn new(register_count: u16) -> Self {
        Self {
            bits: vec![false; usize::from(register_count)],
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
        if let Some(slot) = self.bits.get_mut(usize::from(register)) {
            *slot = true;
        }
    }

    fn contains(&self, register: u16) -> bool {
        self.bits
            .get(usize::from(register))
            .copied()
            .unwrap_or(false)
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
}
