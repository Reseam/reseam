use std::collections::HashMap;

use super::code::{CatchHandler, CodeItem, TryItem, TypedCatch};
use super::instruction::Instruction;
use super::types::TypeIdx;
use crate::error::{invalid, Result};

/// An opaque handle referencing a position in a [`CodeBuilder`] instruction stream.
///
/// Labels are created via [`CodeBuilder::label`] and resolved to concrete
/// code-unit offsets when [`CodeBuilder::build`] is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label(u32);

/// Builds a [`CodeItem`] using symbolic [`Label`]s instead of raw byte offsets.
///
/// Branch targets, switch payload targets, and exception handler addresses
/// are all specified as labels. The builder resolves them to concrete offsets
/// during [`build`](CodeBuilder::build).
#[derive(Debug)]
pub struct CodeBuilder {
    registers_size: u16,
    ins_size: u16,
    outs_size: u16,
    entries: Vec<Entry>,
    tries: Vec<TryDef>,
    next_label: u32,
}

#[derive(Debug)]
enum Entry {
    Instruction(Instruction),
    Label(Label),
}

/// A try-catch definition using labels for addresses.
#[derive(Debug)]
struct TryDef {
    start: Label,
    end: Label,
    handlers: Vec<(TypeIdx, Label)>,
    catch_all: Option<Label>,
}

impl CodeBuilder {
    pub fn new(registers_size: u16, ins_size: u16, outs_size: u16) -> Self {
        Self {
            registers_size,
            ins_size,
            outs_size,
            entries: Vec::new(),
            tries: Vec::new(),
            next_label: 0,
        }
    }

    /// Creates a new unbound label.
    pub fn label(&mut self) -> Label {
        let l = Label(self.next_label);
        self.next_label += 1;
        l
    }

    /// Creates a new label and immediately binds it to the current position.
    pub fn bind_label(&mut self) -> Label {
        let l = self.label();
        self.bind(l);
        l
    }

    /// Binds a previously created label to the current position.
    pub fn bind(&mut self, label: Label) {
        self.entries.push(Entry::Label(label));
    }

    /// Appends an instruction.
    pub fn insn(&mut self, instruction: Instruction) -> &mut Self {
        self.entries.push(Entry::Instruction(instruction));
        self
    }

    /// Appends a goto targeting the given label.
    /// The appropriate goto width is chosen during build.
    pub fn goto(&mut self, target: Label) -> &mut Self {
        self.entries
            .push(Entry::Instruction(Instruction::Goto32 {
                offset: -(target.0 as i32),
            }));
        self
    }

    /// Adds a try-catch region defined by label bounds.
    pub fn add_try(
        &mut self,
        start: Label,
        end: Label,
        handlers: Vec<(TypeIdx, Label)>,
        catch_all: Option<Label>,
    ) -> &mut Self {
        self.tries.push(TryDef {
            start,
            end,
            handlers,
            catch_all,
        });
        self
    }

    /// Resolves all labels and produces a [`CodeItem`].
    pub fn build(self) -> Result<CodeItem> {
        let mut label_addrs: HashMap<Label, u32> = HashMap::new();
        let mut instructions = Vec::new();
        let mut addr: u32 = 0;

        for entry in &self.entries {
            match entry {
                Entry::Label(label) => {
                    label_addrs.insert(*label, addr);
                }
                Entry::Instruction(insn) => {
                    instructions.push(insn.clone());
                    addr += insn.code_units();
                }
            }
        }

        resolve_branches(&mut instructions, &label_addrs)?;

        let mut tries = Vec::new();
        let mut catch_handlers = Vec::new();

        for try_def in &self.tries {
            let start_addr = resolve_label(try_def.start, &label_addrs)?;
            let end_addr = resolve_label(try_def.end, &label_addrs)?;

            if end_addr < start_addr {
                return Err(invalid(
                    "code builder",
                    "try end label precedes start label",
                ));
            }

            let mut typed_catches = Vec::new();
            for (type_idx, handler_label) in &try_def.handlers {
                typed_catches.push(TypedCatch {
                    exception_type: *type_idx,
                    addr: resolve_label(*handler_label, &label_addrs)?,
                });
            }

            let catch_all_addr = try_def
                .catch_all
                .map(|l| resolve_label(l, &label_addrs))
                .transpose()?;

            let handler_idx = catch_handlers.len();
            catch_handlers.push(CatchHandler {
                typed_catches,
                catch_all_addr,
            });

            tries.push(TryItem {
                start_addr,
                insn_count: (end_addr - start_addr) as u16,
                handler_idx,
            });
        }

        Ok(CodeItem {
            registers_size: self.registers_size,
            ins_size: self.ins_size,
            outs_size: self.outs_size,
            debug_info: None,
            instructions,
            tries,
            catch_handlers,
        })
    }
}

fn resolve_label(label: Label, addrs: &HashMap<Label, u32>) -> Result<u32> {
    addrs
        .get(&label)
        .copied()
        .ok_or_else(|| invalid("code builder", format!("unbound label {:?}", label)))
}

fn resolve_branches(
    instructions: &mut [Instruction],
    label_addrs: &HashMap<Label, u32>,
) -> Result<()> {
    let mut addr: u32 = 0;
    for insn in instructions.iter_mut() {
        let cur_addr = addr;
        addr += insn.code_units();
        resolve_insn_labels(insn, cur_addr, label_addrs)?;
    }
    Ok(())
}

fn resolve_insn_labels(
    insn: &mut Instruction,
    cur_addr: u32,
    label_addrs: &HashMap<Label, u32>,
) -> Result<()> {
    match insn {
        Instruction::Goto { offset } => {
            if let Some(target) = label_from_marker(*offset as i32) {
                let target_addr = resolve_label(target, label_addrs)?;
                *offset = (target_addr as i32 - cur_addr as i32) as i8;
            }
        }
        Instruction::Goto16 { offset } => {
            if let Some(target) = label_from_marker(*offset as i32) {
                let target_addr = resolve_label(target, label_addrs)?;
                *offset = (target_addr as i32 - cur_addr as i32) as i16;
            }
        }
        Instruction::Goto32 { offset } => {
            if let Some(target) = label_from_marker(*offset) {
                let target_addr = resolve_label(target, label_addrs)?;
                *offset = target_addr as i32 - cur_addr as i32;
            }
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
            if let Some(target) = label_from_marker(*offset as i32) {
                let target_addr = resolve_label(target, label_addrs)?;
                *offset = (target_addr as i32 - cur_addr as i32) as i16;
            }
        }
        Instruction::PackedSwitch { payload_offset, .. }
        | Instruction::SparseSwitch { payload_offset, .. }
        | Instruction::FillArrayData { payload_offset, .. } => {
            if let Some(target) = label_from_marker(*payload_offset) {
                let target_addr = resolve_label(target, label_addrs)?;
                *payload_offset = target_addr as i32 - cur_addr as i32;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Extracts a label from a negative marker offset.
/// Labels are encoded as `-(label_id)` by the builder methods.
fn label_from_marker(offset: i32) -> Option<Label> {
    if offset <= 0 {
        Some(Label((-offset) as u32))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_goto_forward() {
        let mut b = CodeBuilder::new(1, 0, 0);
        let end = b.label();
        b.goto(end);
        b.insn(Instruction::Nop);
        b.bind(end);
        b.insn(Instruction::ReturnVoid);

        let code = b.build().expect("build should succeed");
        assert_eq!(code.instructions.len(), 3);
        match &code.instructions[0] {
            Instruction::Goto32 { offset } => assert_eq!(*offset, 4),
            other => panic!("expected Goto32, got {other:?}"),
        }
    }

    #[test]
    fn simple_goto_backward() {
        let mut b = CodeBuilder::new(1, 0, 0);
        let top = b.bind_label();
        b.insn(Instruction::Nop);
        b.goto(top);

        let code = b.build().expect("build should succeed");
        match &code.instructions[1] {
            Instruction::Goto32 { offset } => assert_eq!(*offset, -1),
            other => panic!("expected Goto32, got {other:?}"),
        }
    }

    #[test]
    fn if_branch_with_label() {
        let mut b = CodeBuilder::new(2, 0, 0);
        let target = b.label();
        b.insn(Instruction::IfEqz {
            a: 0,
            offset: -(target.0 as i16),
        });
        b.insn(Instruction::ReturnVoid);
        b.bind(target);
        b.insn(Instruction::Nop);
        b.insn(Instruction::ReturnVoid);

        let code = b.build().expect("build should succeed");
        match &code.instructions[0] {
            Instruction::IfEqz { offset, .. } => assert_eq!(*offset, 3),
            other => panic!("expected IfEqz, got {other:?}"),
        }
    }

    #[test]
    fn try_catch_with_labels() {
        let mut b = CodeBuilder::new(2, 0, 0);
        let try_start = b.bind_label();
        b.insn(Instruction::Nop);
        let try_end = b.bind_label();
        b.insn(Instruction::ReturnVoid);
        let handler = b.bind_label();
        b.insn(Instruction::MoveException { dest: 0 });
        b.insn(Instruction::ReturnVoid);

        b.add_try(
            try_start,
            try_end,
            vec![(TypeIdx(0), handler)],
            None,
        );

        let code = b.build().expect("build should succeed");
        assert_eq!(code.tries.len(), 1);
        assert_eq!(code.tries[0].start_addr, 0);
        assert_eq!(code.tries[0].insn_count, 1);
        assert_eq!(code.catch_handlers[0].typed_catches[0].addr, 2);
    }

    #[test]
    fn unbound_label_returns_error() {
        let mut b = CodeBuilder::new(1, 0, 0);
        let dangling = b.label();
        b.goto(dangling);

        let err = b.build().unwrap_err();
        assert!(err.to_string().contains("unbound label"));
    }
}
