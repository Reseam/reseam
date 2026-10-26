// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::instruction::Instruction;
use smallvec::SmallVec;

impl Instruction {
    /// Operand starting registers in encoding order; wide operands occur once.
    pub fn registers_used(&self) -> SmallVec<[u16; 6]> {
        let mut registers = SmallVec::new();
        self.visit_operands(|operand| registers.push(operand.register));
        registers
    }
}
