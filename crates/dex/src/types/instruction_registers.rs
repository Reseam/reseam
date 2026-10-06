// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::instruction::Instruction;
use super::register_operands::Access;

impl Instruction {
    pub fn visit_read_registers(&self, mut visit: impl FnMut(u16)) {
        self.visit_operands(|operand| {
            if operand.access != Access::Write {
                for word in 0..operand.kind.words() {
                    if let Some(register) = operand.register.checked_add(word) {
                        visit(register);
                    }
                }
            }
        });
    }

    /// Visits value definitions; a check-cast only refines the existing value's type.
    pub fn visit_written_registers(&self, mut visit: impl FnMut(u16)) {
        self.visit_operands(|operand| {
            if operand.access.writes_value() {
                for word in 0..operand.kind.words() {
                    if let Some(register) = operand.register.checked_add(word) {
                        visit(register);
                    }
                }
            }
        });
    }
}
