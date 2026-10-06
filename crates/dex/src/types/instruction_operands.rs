// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::instruction::Instruction;
use super::register_operands::Access;
use smallvec::SmallVec;

impl Instruction {
    /// The first destination, including a type refinement by check-cast.
    pub fn dest_register(&self) -> Option<u16> {
        let mut destination = None;
        self.visit_operands(|operand| {
            if operand.access != Access::Read && destination.is_none() {
                destination = Some(operand.register);
            }
        });
        destination
    }

    /// The first register whose value is defined by this instruction.
    pub fn write_register(&self) -> Option<u16> {
        let mut destination = None;
        self.visit_operands(|operand| {
            if operand.access.writes_value() && destination.is_none() {
                destination = Some(operand.register);
            }
        });
        destination
    }

    /// Operand starting registers in encoding order; wide operands occur once.
    pub fn registers_used(&self) -> SmallVec<[u16; 6]> {
        let mut registers = SmallVec::new();
        self.visit_operands(|operand| registers.push(operand.register));
        registers
    }
}
