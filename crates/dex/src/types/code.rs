use super::debug::DebugInfo;
use super::instruction::Instruction;
use super::TypeIdx;

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
    pub fn instruction(&self, index: usize) -> &Instruction {
        &self.instructions[index]
    }

    pub fn replace_instruction(&mut self, index: usize, insn: Instruction) {
        self.instructions[index] = insn;
    }

    pub fn insert_instruction(&mut self, index: usize, insn: Instruction) {
        let delta = insn.code_units() as i32;
        let insert_addr = self.code_unit_offset(index);
        self.instructions.insert(index, insn);
        self.fixup_offsets(insert_addr, delta);
    }

    pub fn insert_instructions(&mut self, index: usize, insns: &[Instruction]) {
        let delta: i32 = insns.iter().map(|i| i.code_units() as i32).sum();
        let insert_addr = self.code_unit_offset(index);
        self.instructions
            .splice(index..index, insns.iter().cloned());
        self.fixup_offsets(insert_addr, delta);
    }

    pub fn remove_instruction(&mut self, index: usize) {
        let delta = -(self.instructions[index].code_units() as i32);
        let remove_addr = self.code_unit_offset(index);
        self.instructions.remove(index);
        self.fixup_offsets(remove_addr, delta);
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

    fn fixup_offsets(&mut self, addr: u32, delta: i32) {
        let mut cur_addr: u32 = 0;
        for insn in &mut self.instructions {
            let insn_size = insn.code_units();
            fixup_branch(insn, cur_addr, addr, delta);
            cur_addr += insn_size;
        }

        for t in &mut self.tries {
            let try_end = t.start_addr + t.insn_count as u32;
            if t.start_addr >= addr {
                t.start_addr = (t.start_addr as i32 + delta) as u32;
            } else if try_end > addr {
                t.insn_count = (t.insn_count as i32 + delta) as u16;
            }
        }

        for handler in &mut self.catch_handlers {
            for tc in &mut handler.typed_catches {
                if tc.addr >= addr {
                    tc.addr = (tc.addr as i32 + delta) as u32;
                }
            }
            if let Some(ref mut catch_all) = handler.catch_all_addr {
                if *catch_all >= addr {
                    *catch_all = (*catch_all as i32 + delta) as u32;
                }
            }
        }
    }
}

fn fixup_branch(insn: &mut Instruction, insn_addr: u32, change_addr: u32, delta: i32) {
    match insn {
        Instruction::Goto { offset } => {
            *offset = fixup_i8(*offset as i32, insn_addr, change_addr, delta);
        }
        Instruction::Goto16 { offset } => {
            *offset = fixup_i16(*offset as i32, insn_addr, change_addr, delta);
        }
        Instruction::Goto32 { offset } => {
            *offset = fixup_i32(*offset, insn_addr, change_addr, delta);
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
            *offset = fixup_i16(*offset as i32, insn_addr, change_addr, delta);
        }
        Instruction::PackedSwitch { payload_offset, .. }
        | Instruction::SparseSwitch { payload_offset, .. }
        | Instruction::FillArrayData { payload_offset, .. } => {
            *payload_offset = fixup_i32(*payload_offset, insn_addr, change_addr, delta);
        }
        Instruction::PackedSwitchPayload { .. } | Instruction::SparseSwitchPayload { .. } => {}
        _ => {}
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

fn fixup_i16(offset: i32, insn_addr: u32, change_addr: u32, delta: i32) -> i16 {
    fixup_i32(offset, insn_addr, change_addr, delta) as i16
}

fn fixup_i8(offset: i32, insn_addr: u32, change_addr: u32, delta: i32) -> i8 {
    fixup_i32(offset, insn_addr, change_addr, delta) as i8
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
