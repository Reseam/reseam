// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use reseam_dex::{self as dex, CodeItem, IncomingBranches, Instruction as DexInsn, RegisterKind};

use super::super::logged;
use super::super::mutation::{edit_mapping, growth_mapping};
use super::{Code, Frame, kind};
use crate::bridge::dex::types::{CodeEdit, CodeRegister};

impl Code<'_> {
    /// The code before each return, reading parameters as the method was entered: copied at
    /// entry into locals added for them, which the copies keep live until the returns.
    pub(super) fn before_returns(&self, body: &mut CodeItem) -> Result<Vec<CodeEdit>, String> {
        let returns = body
            .instructions()
            .iter()
            .filter(|instruction| instruction.is_return())
            .count();
        if returns == 0 {
            return Ok(Vec::new());
        }
        let parameters: BTreeMap<u16, RegisterKind> = self
            .operands()
            .filter_map(|operand| match operand.register {
                CodeRegister::Parameter(offset) => Some((offset, kind(operand))),
                _ => None,
            })
            .collect();
        let base = body.registers_size() - body.ins_size();
        let mut words = 0;
        let copies: BTreeMap<u16, u16> = parameters
            .iter()
            .map(|(&offset, kind)| {
                let copy = base + words;
                words += kind.words();
                (offset, copy)
            })
            .collect();
        let mut edits = Vec::new();
        if words != 0 {
            let map = logged(
                "add parameter copies",
                dex::grow_registers(body, words, &self.incoming, &self.protected, self.dex),
            )?;
            edits.push(CodeEdit::Grow(growth_mapping(base, words, map)?));
        }
        let protected: Vec<u16> = self
            .protected
            .iter()
            .copied()
            .chain(base..base + words)
            .collect();
        for ordinal in (0..returns).rev() {
            let (index, returned) = body
                .instructions()
                .iter()
                .enumerate()
                .filter(|(_, instruction)| instruction.is_return())
                .nth(ordinal)
                .map(|(index, instruction)| (index, instruction.registers_used().first().copied()))
                .expect("inserting before a return keeps every return");
            let frame = Frame::Copies {
                copies: &copies,
                returned,
            };
            let block = self.block(&frame)?;
            edits.extend(self.place(body, index, IncomingBranches::Enter, &block, &protected)?);
        }
        if words != 0 {
            let incoming = body.registers_size() - body.ins_size();
            let moves: Vec<DexInsn> = parameters
                .iter()
                .map(|(offset, &kind)| dex::move_register(copies[offset], incoming + offset, kind))
                .collect();
            let map = logged("copy parameters", body.insert_instructions(0, &moves))?;
            edits.push(CodeEdit::Insert(edit_mapping(map)?));
        }
        Ok(edits)
    }
}
