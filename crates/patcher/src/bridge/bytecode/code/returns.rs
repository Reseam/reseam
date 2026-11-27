// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::{CodeItem, IncomingBranches};

use super::{Code, Frame};
use crate::bridge::dex::types::CodeEdit;

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
        self.with_entry_copies(body, &self.parameters(), |body, copies, protected, _| {
            let mut edits = Vec::new();
            for ordinal in (0..returns).rev() {
                let (index, returned) = body
                    .instructions()
                    .iter()
                    .enumerate()
                    .filter(|(_, instruction)| instruction.is_return())
                    .nth(ordinal)
                    .map(|(index, instruction)| {
                        (index, instruction.registers_used().first().copied())
                    })
                    .expect("inserting before a return keeps every return");
                let frame = Frame::Copies { copies, returned };
                let block = self.block(&frame)?;
                edits.extend(self.place(
                    body,
                    index,
                    IncomingBranches::Enter,
                    &block,
                    protected,
                )?);
            }
            Ok(edits)
        })
    }
}
