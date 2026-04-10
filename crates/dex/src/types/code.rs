use super::debug::DebugInfo;
use super::instruction::Instruction;
use super::TypeIdx;
use crate::error::Result;

#[derive(Debug, Clone)]
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
    pub fn compute_outs_size(&self) -> u16 {
        self.instructions
            .iter()
            .map(|i| i.outgoing_arg_count())
            .max()
            .unwrap_or(0)
    }

    pub fn instruction(&self, index: usize) -> &Instruction {
        &self.instructions[index]
    }

    pub fn replace_instruction(&mut self, index: usize, insn: Instruction) -> Result<()> {
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
            if cur > addr {
                if matches!(
                    insn,
                    Instruction::PackedSwitchPayload { .. }
                        | Instruction::SparseSwitchPayload { .. }
                        | Instruction::FillArrayDataPayload { .. }
                ) {
                    return true;
                }
            }
            cur += insn.code_units();
        }
        false
    }

    fn fixup_offsets(&mut self, addr: u32, delta: i32) -> Result<()> {
        let mut total_growth: i32 = 0;
        let mut cur_addr: u32 = 0;
        for insn in &mut self.instructions {
            let growth = fixup_branch(insn, cur_addr, addr, delta + total_growth)?;
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

/// Returns additional code-unit growth if a goto was promoted to a wider form.
fn fixup_branch(
    insn: &mut Instruction,
    insn_addr: u32,
    change_addr: u32,
    delta: i32,
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
        | Instruction::SparseSwitch { payload_offset, .. }
        | Instruction::FillArrayData { payload_offset, .. } => {
            *payload_offset = fixup_i32(*payload_offset, insn_addr, change_addr, delta);
            Ok(0)
        }
        Instruction::PackedSwitchPayload { .. } | Instruction::SparseSwitchPayload { .. } => Ok(0),
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

#[derive(Debug, Clone)]
pub struct TryItem {
    pub start_addr: u32,
    pub insn_count: u16,
    pub handler_idx: usize,
}

#[derive(Debug, Clone)]
pub struct CatchHandler {
    pub typed_catches: Vec<TypedCatch>,
    pub catch_all_addr: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TypedCatch {
    pub exception_type: TypeIdx,
    pub addr: u32,
}
