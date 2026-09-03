// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use super::debug::DebugInfo;
use super::instruction::Instruction;
use super::TypeIdx;
use crate::error::invalid;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct CodeItem {
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub debug_info: Option<DebugInfo>,
    pub instructions: Vec<Instruction>,
    pub tries: Vec<TryItem>,
    pub catch_handlers: Vec<CatchHandler>,
}

impl CodeItem {
    /// Computes `outs_size` from the actual instructions (max outgoing arg count).
    /// Outgoing argument words the code needs: what the instructions require
    /// after any edits, never less than what the method was compiled with.
    pub fn compute_outs_size(&self) -> u16 {
        self.instructions
            .iter()
            .map(|i| i.outgoing_arg_count())
            .max()
            .unwrap_or(0)
            .max(self.outs_size)
    }

    pub fn instruction(&self, index: usize) -> &Instruction {
        &self.instructions[index]
    }

    pub fn replace_instruction(&mut self, index: usize, insn: Instruction) -> Result<()> {
        self.ensure_existing_instruction(index)?;
        let old_size = self.instructions[index].code_units() as i32;
        let new_size = insn.code_units() as i32;
        let mut delta = new_size - old_size;
        let change_addr = self.code_unit_offset(index);
        self.instructions[index] = insn;
        if delta != 0 {
            let needs_pad = delta % 2 != 0 && self.has_payload_after(change_addr);
            if needs_pad {
                self.instructions.insert(index + 1, Instruction::Nop);
                delta += 1;
            }
            self.fixup_offsets(change_addr + old_size as u32, delta)?;
        }
        Ok(())
    }

    pub fn insert_instruction(&mut self, index: usize, insn: Instruction) -> Result<()> {
        self.insert_instructions(index, &[insn])
    }

    pub fn insert_instructions(&mut self, index: usize, insns: &[Instruction]) -> Result<()> {
        self.ensure_insert_index(index)?;
        let mut delta: i32 = insns.iter().map(|i| i.code_units() as i32).sum();
        let insert_addr = self.code_unit_offset(index);

        let needs_pad = delta % 2 != 0 && self.has_payload_after(insert_addr);

        self.instructions
            .splice(index..index, insns.iter().cloned());

        if needs_pad {
            let pad_pos = index + insns.len();
            self.instructions.insert(pad_pos, Instruction::Nop);
            delta += 1;
        }

        self.fixup_offsets(insert_addr, delta)
    }

    pub fn remove_instruction(&mut self, index: usize) -> Result<()> {
        self.ensure_existing_instruction(index)?;
        let delta = -(self.instructions[index].code_units() as i32);
        let remove_addr = self.code_unit_offset(index);
        self.instructions.remove(index);
        self.fixup_offsets(remove_addr, delta)
    }

    pub fn set_instructions(&mut self, insns: Vec<Instruction>) {
        self.instructions = insns;
        self.tries.clear();
        self.catch_handlers.clear();
    }

    pub fn return_early(&mut self) {
        self.set_instructions(vec![Instruction::ReturnVoid]);
        self.registers_size = self.ins_size;
        self.outs_size = 0;
        self.debug_info = None;
    }

    pub fn return_early_int(&mut self, value: i32) {
        self.set_instructions(vec![
            Instruction::Const { dest: 0, value },
            Instruction::Return { src: 0 },
        ]);
        self.registers_size = self.ins_size.max(1);
        self.outs_size = 0;
        self.debug_info = None;
    }

    pub fn return_early_object(&mut self, value: i32) {
        self.set_instructions(vec![
            Instruction::Const { dest: 0, value },
            Instruction::ReturnObject { src: 0 },
        ]);
        self.registers_size = self.ins_size.max(1);
        self.outs_size = 0;
        self.debug_info = None;
    }

    pub fn return_early_wide(&mut self, value: i64) {
        self.set_instructions(vec![
            Instruction::ConstWide { dest: 0, value },
            Instruction::ReturnWide { src: 0 },
        ]);
        self.registers_size = self.ins_size.max(2);
        self.outs_size = 0;
        self.debug_info = None;
    }

    fn code_unit_offset(&self, index: usize) -> u32 {
        self.instructions[..index]
            .iter()
            .map(|i| i.code_units())
            .sum()
    }

    fn ensure_insert_index(&self, index: usize) -> Result<()> {
        if index <= self.instructions.len() {
            Ok(())
        } else {
            Err(invalid(
                "code item",
                format!(
                    "insert index {index} is out of bounds for instruction count {}",
                    self.instructions.len()
                ),
            ))
        }
    }

    fn ensure_existing_instruction(&self, index: usize) -> Result<()> {
        if index < self.instructions.len() {
            Ok(())
        } else {
            Err(invalid(
                "code item",
                format!(
                    "instruction index {index} is out of bounds for instruction count {}",
                    self.instructions.len()
                ),
            ))
        }
    }

    fn has_payload_after(&self, addr: u32) -> bool {
        let has_any_payload = self.instructions.iter().any(|insn| {
            matches!(
                insn,
                Instruction::PackedSwitchPayload { .. }
                    | Instruction::SparseSwitchPayload { .. }
                    | Instruction::FillArrayDataPayload { .. }
            )
        });
        if !has_any_payload {
            return false;
        }
        let mut cur: u32 = 0;
        for insn in &self.instructions {
            if cur > addr
                && matches!(
                    insn,
                    Instruction::PackedSwitchPayload { .. }
                        | Instruction::SparseSwitchPayload { .. }
                        | Instruction::FillArrayDataPayload { .. }
                )
            {
                return true;
            }
            cur += insn.code_units();
        }
        false
    }

    fn fixup_offsets(&mut self, addr: u32, delta: i32) -> Result<()> {
        let mut switch_bases: HashMap<u32, SwitchBase> = HashMap::new();
        let mut total_growth: i32 = 0;
        let mut cur_addr: u32 = 0;
        for insn in &mut self.instructions {
            let growth = fixup_branch(insn, cur_addr, addr, delta + total_growth, &mut switch_bases)?;
            total_growth += growth;
            cur_addr += insn.code_units();
        }

        let effective_delta = delta + total_growth;

        for t in &mut self.tries {
            let try_end = t.start_addr + t.insn_count as u32;
            if t.start_addr >= addr {
                t.start_addr = (t.start_addr as i32 + effective_delta) as u32;
            } else if try_end > addr {
                t.insn_count = (t.insn_count as i32 + effective_delta) as u16;
            }
        }

        for handler in &mut self.catch_handlers {
            for tc in &mut handler.typed_catches {
                if tc.addr >= addr {
                    tc.addr = (tc.addr as i32 + effective_delta) as u32;
                }
            }
            if let Some(ref mut catch_all) = handler.catch_all_addr {
                if *catch_all >= addr {
                    *catch_all = (*catch_all as i32 + effective_delta) as u32;
                }
            }
        }
        Ok(())
    }
}

/// Address and relocation context of a switch instruction, keyed by the address of
/// the payload it references. A switch payload encodes its case targets relative to
/// the switch instruction, not to the payload, so the payload is relocated using the
/// switch's address and the delta that applied when the switch itself was relocated.
struct SwitchBase {
    addr: u32,
    delta: i32,
}

/// Returns additional code-unit growth if a goto was promoted to a wider form.
fn fixup_branch(
    insn: &mut Instruction,
    insn_addr: u32,
    change_addr: u32,
    delta: i32,
    switch_bases: &mut HashMap<u32, SwitchBase>,
) -> Result<i32> {
    match insn {
        Instruction::Goto { offset } => {
            let new_offset = fixup_i32(*offset as i32, insn_addr, change_addr, delta);
            if let Ok(v) = i8::try_from(new_offset) {
                *offset = v;
                Ok(0)
            } else if let Ok(v) = i16::try_from(new_offset) {
                *insn = Instruction::Goto16 { offset: v };
                Ok(1) // Goto16 is 2 code units vs Goto's 1
            } else {
                *insn = Instruction::Goto32 { offset: new_offset };
                Ok(2) // Goto32 is 3 code units vs Goto's 1
            }
        }
        Instruction::Goto16 { offset } => {
            let new_offset = fixup_i32(*offset as i32, insn_addr, change_addr, delta);
            if let Ok(v) = i16::try_from(new_offset) {
                *offset = v;
                Ok(0)
            } else {
                *insn = Instruction::Goto32 { offset: new_offset };
                Ok(1) // Goto32 is 3 code units vs Goto16's 2
            }
        }
        Instruction::Goto32 { offset } => {
            *offset = fixup_i32(*offset, insn_addr, change_addr, delta);
            Ok(0)
        }
        Instruction::IfEq { offset, .. }
        | Instruction::IfNe { offset, .. }
        | Instruction::IfLt { offset, .. }
        | Instruction::IfGe { offset, .. }
        | Instruction::IfGt { offset, .. }
        | Instruction::IfLe { offset, .. }
        | Instruction::IfEqz { offset, .. }
        | Instruction::IfNez { offset, .. }
        | Instruction::IfLtz { offset, .. }
        | Instruction::IfGez { offset, .. }
        | Instruction::IfGtz { offset, .. }
        | Instruction::IfLez { offset, .. } => {
            let new_offset = fixup_i32(*offset as i32, insn_addr, change_addr, delta);
            match i16::try_from(new_offset) {
                Ok(v) => {
                    *offset = v;
                    Ok(0)
                }
                Err(_) => Err(crate::error::invalid(
                    "if-branch fixup",
                    format!(
                        "adjusted offset {new_offset} at address {insn_addr} exceeds i16 range"
                    ),
                )),
            }
        }
        Instruction::PackedSwitch { payload_offset, .. }
        | Instruction::SparseSwitch { payload_offset, .. } => {
            *payload_offset = fixup_i32(*payload_offset, insn_addr, change_addr, delta);
            let payload_addr = (insn_addr as i32 + *payload_offset) as u32;
            switch_bases.insert(
                payload_addr,
                SwitchBase {
                    addr: insn_addr,
                    delta,
                },
            );
            Ok(0)
        }
        Instruction::FillArrayData { payload_offset, .. } => {
            *payload_offset = fixup_i32(*payload_offset, insn_addr, change_addr, delta);
            Ok(0)
        }
        Instruction::PackedSwitchPayload(payload) => {
            if let Some(base) = switch_bases.get(&insn_addr) {
                for target in payload.targets.iter_mut() {
                    *target = fixup_i32(*target, base.addr, change_addr, base.delta);
                }
            }
            Ok(0)
        }
        Instruction::SparseSwitchPayload(payload) => {
            if let Some(base) = switch_bases.get(&insn_addr) {
                for (_, target) in payload.keys_and_targets.iter_mut() {
                    *target = fixup_i32(*target, base.addr, change_addr, base.delta);
                }
            }
            Ok(0)
        }
        _ => Ok(0),
    }
}

fn fixup_i32(offset: i32, insn_addr: u32, change_addr: u32, delta: i32) -> i32 {
    let target = insn_addr as i32 + offset;
    let iaddr = insn_addr as i32;
    let caddr = change_addr as i32;

    if offset > 0 {
        if caddr > iaddr && caddr <= target {
            offset + delta
        } else {
            offset
        }
    } else {
        if caddr > target && caddr <= iaddr {
            offset - delta
        } else {
            offset
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TryItem {
    pub start_addr: u32,
    pub insn_count: u16,
    pub handler_idx: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchHandler {
    pub typed_catches: Vec<TypedCatch>,
    pub catch_all_addr: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedCatch {
    pub exception_type: TypeIdx,
    pub addr: u32,
}

#[cfg(test)]
mod tests {
    use super::{CatchHandler, CodeItem, Instruction, TryItem};

    fn code_item() -> CodeItem {
        CodeItem {
            registers_size: 0,
            ins_size: 0,
            outs_size: 0,
            debug_info: None,
            instructions: vec![Instruction::ReturnVoid],
            tries: Vec::<TryItem>::new(),
            catch_handlers: Vec::<CatchHandler>::new(),
        }
    }

    #[test]
    fn insert_instruction_rejects_out_of_bounds_index() {
        let mut code = code_item();
        let err = code
            .insert_instruction(2, Instruction::Nop)
            .expect_err("out-of-bounds insert should fail");
        assert!(err.to_string().contains("insert index 2"));
    }

    #[test]
    fn replace_instruction_rejects_out_of_bounds_index() {
        let mut code = code_item();
        let err = code
            .replace_instruction(1, Instruction::Nop)
            .expect_err("out-of-bounds replace should fail");
        assert!(err.to_string().contains("instruction index 1"));
    }

    #[test]
    fn remove_instruction_rejects_out_of_bounds_index() {
        let mut code = code_item();
        let err = code
            .remove_instruction(1)
            .expect_err("out-of-bounds remove should fail");
        assert!(err.to_string().contains("instruction index 1"));
    }

    /// Packed switch at addr 0; case targets point at the Nops at addr 3 and 5.
    fn packed_switch_method() -> CodeItem {
        CodeItem {
            registers_size: 1,
            ins_size: 0,
            outs_size: 0,
            debug_info: None,
            instructions: vec![
                Instruction::PackedSwitch {
                    test: 0,
                    payload_offset: 8,
                },
                Instruction::Nop,
                Instruction::ReturnVoid,
                Instruction::Nop,
                Instruction::ReturnVoid,
                Instruction::Nop,
                Instruction::PackedSwitchPayload(Box::new(
                    crate::types::instruction::PackedSwitchData {
                        first_key: 0,
                        targets: vec![3, 5],
                    },
                )),
            ],
            tries: Vec::new(),
            catch_handlers: Vec::new(),
        }
    }

    fn packed_targets(code: &CodeItem) -> &Vec<i32> {
        match code.instructions.last().expect("payload present") {
            Instruction::PackedSwitchPayload(p) => &p.targets,
            other => panic!("expected packed payload, got {other:?}"),
        }
    }

    fn payload_offset(code: &CodeItem) -> i32 {
        match &code.instructions[0] {
            Instruction::PackedSwitch { payload_offset, .. }
            | Instruction::SparseSwitch { payload_offset, .. } => *payload_offset,
            other => panic!("expected switch, got {other:?}"),
        }
    }

    #[test]
    fn packed_switch_targets_relocate_when_insert_precedes_them() {
        let mut code = packed_switch_method();
        code.insert_instructions(1, &[Instruction::Nop, Instruction::Nop])
            .expect("insert");
        assert_eq!(payload_offset(&code), 10);
        assert_eq!(packed_targets(&code), &vec![5, 7]);
    }

    #[test]
    fn packed_switch_target_before_insert_is_unchanged() {
        let mut code = packed_switch_method();
        // Insert at addr 5: the addr-3 case stays, the addr-5 case shifts by 2.
        code.insert_instructions(3, &[Instruction::Nop, Instruction::Nop])
            .expect("insert");
        assert_eq!(payload_offset(&code), 10);
        assert_eq!(packed_targets(&code), &vec![3, 7]);
    }

    #[test]
    fn packed_switch_targets_relocate_on_remove() {
        let mut code = packed_switch_method();
        // Remove the ReturnVoid at addr 4: both later case targets shift down by 1.
        code.remove_instruction(2).expect("remove");
        assert_eq!(payload_offset(&code), 7);
        assert_eq!(packed_targets(&code), &vec![3, 4]);
    }

    #[test]
    fn sparse_switch_targets_relocate_when_insert_precedes_them() {
        let mut code = CodeItem {
            registers_size: 1,
            ins_size: 0,
            outs_size: 0,
            debug_info: None,
            instructions: vec![
                Instruction::SparseSwitch {
                    test: 0,
                    payload_offset: 8,
                },
                Instruction::Nop,
                Instruction::ReturnVoid,
                Instruction::Nop,
                Instruction::ReturnVoid,
                Instruction::Nop,
                Instruction::SparseSwitchPayload(Box::new(
                    crate::types::instruction::SparseSwitchData {
                        keys_and_targets: vec![(10, 3), (20, 5)],
                    },
                )),
            ],
            tries: Vec::new(),
            catch_handlers: Vec::new(),
        };
        code.insert_instructions(1, &[Instruction::Nop, Instruction::Nop])
            .expect("insert");
        assert_eq!(payload_offset(&code), 10);
        match code.instructions.last().expect("payload") {
            Instruction::SparseSwitchPayload(p) => {
                assert_eq!(p.keys_and_targets, vec![(10, 5), (20, 7)]);
            }
            other => panic!("expected sparse payload, got {other:?}"),
        }
    }
}
