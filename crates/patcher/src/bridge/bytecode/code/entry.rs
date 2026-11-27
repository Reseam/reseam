// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use reseam_dex::{self as dex, CodeItem, Instruction as DexInsn, InstructionMap, RegisterKind};

use super::super::logged;
use super::super::mutation::{edit_mapping, growth_mapping};
use super::{Code, kind};
use crate::bridge::dex::types::{CodeEdit, CodeRegister};
use crate::bridge::trace;
use crate::context::Moved;

/// Parameters copied at entry into locals added for them. Adding them moved every register from
/// `base` up by `words`, so a register the code names from before then has moved too.
pub(super) struct EntryCopies {
    copies: BTreeMap<u16, u16>,
    base: u16,
    words: u16,
}

impl EntryCopies {
    pub(super) fn copy(&self, offset: u16) -> u16 {
        self.copies[&offset]
    }

    pub(super) fn moved(&self, register: u16) -> u16 {
        if register >= self.base {
            register + self.words
        } else {
            register
        }
    }
}

impl Code<'_> {
    pub(super) fn parameters(&self) -> BTreeMap<u16, RegisterKind> {
        self.operands()
            .filter_map(|operand| match operand.register {
                CodeRegister::Parameter(offset) => Some((offset, kind(operand))),
                _ => None,
            })
            .collect()
    }

    pub(super) fn with_entry_copies(
        &self,
        body: &mut CodeItem,
        parameters: &BTreeMap<u16, RegisterKind>,
        place: impl FnOnce(
            &mut CodeItem,
            &EntryCopies,
            &[u16],
            &InstructionMap,
        ) -> Result<Vec<CodeEdit>, String>,
    ) -> Result<Vec<CodeEdit>, String> {
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
        let moved = logged(
            "add parameter copies",
            dex::grow_registers(body, words, &self.incoming, &self.protected, self.dex),
        )?;
        if words != 0 {
            trace::moved(self.handle, &moved, Moved::Rewrite);
            edits.push(CodeEdit::Grow(growth_mapping(base, words, moved.clone())?));
        }
        let copies = EntryCopies {
            copies,
            base,
            words,
        };
        let protected: Vec<u16> = self
            .protected
            .iter()
            .map(|&register| copies.moved(register))
            .chain(base..base + words)
            .collect();
        edits.extend(place(body, &copies, &protected, &moved)?);
        if words != 0 {
            let incoming = body.registers_size() - body.ins_size();
            let moves: Vec<DexInsn> = parameters
                .iter()
                .map(|(offset, &kind)| {
                    dex::move_register(copies.copy(*offset), incoming + offset, kind)
                })
                .collect();
            let map = logged("copy parameters", body.insert_instructions(0, &moves))?;
            trace::moved(self.handle, &map, Moved::Insertion);
            edits.push(CodeEdit::Insert(edit_mapping(map)?));
        }
        Ok(edits)
    }
}

pub(super) fn overwritten(body: &CodeItem, at: usize, register: u16, kind: RegisterKind) -> bool {
    (register..register + kind.words()).any(|word| {
        dex::reaching_definitions(body, at, word).is_none_or(|(writes, _)| !writes.is_empty())
    })
}
