// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::SmaliLine;
use super::labels::{LabelKind, Labels};
use super::mnemonic::mnemonic;
use super::names::SmaliNames;
use super::operands::opname;
use crate::error::{Result, invalid};
use crate::file::DexFile;
use crate::instruction::{Instruction, RegList};
use crate::model::code::CodeItem;

/// One method body being written as smali.
pub(super) struct Body<'a> {
    pub(super) names: SmaliNames<'a>,
    code: &'a CodeItem,
    labels: Labels,
    /// Registers below this are locals (`vN`); the rest hold parameters (`pN`).
    locals: u16,
}

impl<'a> Body<'a> {
    pub(super) fn new(dex: &'a DexFile, code: &'a CodeItem) -> Result<Self> {
        Ok(Self {
            names: SmaliNames::new(dex),
            code,
            labels: Labels::of(code)?,
            locals: code.registers_size() - code.ins_size(),
        })
    }

    pub(super) fn lines(&self) -> Result<Vec<SmaliLine>> {
        let instructions = self.code.instructions();
        let mut lines = Vec::with_capacity(instructions.len());
        for index in 0..=instructions.len() {
            for label in self.labels.at(index) {
                lines.push(SmaliLine::Label(label.to_string()));
                if label.kind == LabelKind::TryEnd {
                    self.catches(index, &mut lines);
                }
            }
            if let Some(insn) = instructions.get(index) {
                lines.push(SmaliLine::Instruction {
                    index,
                    text: self.instruction(index, insn)?,
                });
            }
        }
        Ok(lines)
    }

    fn instruction(&self, index: usize, insn: &Instruction) -> Result<String> {
        let mnemonic = mnemonic(opname(insn));
        let operands = self.operands(index, insn)?;
        Ok(if operands.is_empty() || operands.starts_with('\n') {
            mnemonic + &operands
        } else {
            format!("{mnemonic} {operands}")
        })
    }

    /// The `.catch` directives of the tries that end before instruction `index`.
    fn catches(&self, index: usize, lines: &mut Vec<SmaliLine>) {
        for item in self.code.tries() {
            let end = item.start_addr + u32::from(item.insn_count);
            if self.labels.index_of(end) != Some(index) {
                continue;
            }
            let start = self.labels.at_address(item.start_addr, LabelKind::TryStart);
            let end_label = self.labels.at_address(end, LabelKind::TryEnd);
            let handler = &self.code.catch_handlers()[item.handler_idx];
            for catch in &handler.typed_catches {
                lines.push(SmaliLine::Directive(format!(
                    ".catch {} {{{start} .. {end_label}}} {}",
                    self.names.type_(catch.exception_type),
                    self.labels.at_address(catch.addr, LabelKind::Catch),
                )));
            }
            if let Some(address) = handler.catch_all_addr {
                lines.push(SmaliLine::Directive(format!(
                    ".catchall {{{start} .. {end_label}}} {}",
                    self.labels.at_address(address, LabelKind::CatchAll),
                )));
            }
        }
    }

    pub(super) fn register(&self, register: u16) -> String {
        match register.checked_sub(self.locals) {
            Some(parameter) => format!("p{parameter}"),
            None => format!("v{register}"),
        }
    }

    pub(super) fn list(&self, args: &RegList) -> String {
        let registers: Vec<String> = args
            .iter()
            .map(|&register| self.register(u16::from(register)))
            .collect();
        format!("{{{}}}", registers.join(", "))
    }

    pub(super) fn range(&self, first: u16, count: u16) -> String {
        match count {
            0 => "{}".to_owned(),
            count => format!(
                "{{{} .. {}}}",
                self.register(first),
                self.register(first + count - 1)
            ),
        }
    }

    /// The label instruction `at` reaches `delta` code units away.
    pub(super) fn label(&self, at: usize, delta: i32, kind: LabelKind) -> String {
        self.labels.reached(at, delta, kind).to_string()
    }

    /// The switch that reads the payload at instruction `payload`.
    pub(super) fn switch_of(&self, payload: usize) -> Result<usize> {
        self.labels.switch_of(payload).ok_or_else(|| {
            invalid(
                "code item",
                format!("no switch reads the payload at instruction {payload}"),
            )
        })
    }
}
