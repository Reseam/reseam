// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::Instruction;
use crate::error::{Result, invalid};

pub(crate) fn is_payload(insn: &Instruction) -> bool {
    matches!(
        insn,
        Instruction::PackedSwitchPayload(_)
            | Instruction::SparseSwitchPayload(_)
            | Instruction::FillArrayDataPayload(_)
    )
}
/// The ordinary branch displacement, excluding payload references.
pub fn branch_offset(insn: &Instruction) -> Option<i32> {
    use Instruction::{
        Goto, Goto16, Goto32, IfEq, IfEqz, IfGe, IfGez, IfGt, IfGtz, IfLe, IfLez, IfLt, IfLtz,
        IfNe, IfNez,
    };
    match insn {
        Goto { offset } => Some(i32::from(*offset)),
        Goto16 { offset }
        | IfEq { offset, .. }
        | IfNe { offset, .. }
        | IfLt { offset, .. }
        | IfGe { offset, .. }
        | IfGt { offset, .. }
        | IfLe { offset, .. }
        | IfEqz { offset, .. }
        | IfNez { offset, .. }
        | IfLtz { offset, .. }
        | IfGez { offset, .. }
        | IfGtz { offset, .. }
        | IfLez { offset, .. } => Some(i32::from(*offset)),
        Goto32 { offset } => Some(*offset),
        _ => None,
    }
}
pub(crate) fn is_condition(insn: &Instruction) -> bool {
    branch_offset(insn).is_some()
        && !matches!(
            insn,
            Instruction::Goto { .. } | Instruction::Goto16 { .. } | Instruction::Goto32 { .. }
        )
}
pub(crate) fn set_branch_offset(insn: &mut Instruction, delta: i32) -> Result<()> {
    use Instruction::{
        Goto, Goto16, Goto32, IfEq, IfEqz, IfGe, IfGez, IfGt, IfGtz, IfLe, IfLez, IfLt, IfLtz,
        IfNe, IfNez,
    };
    match insn {
        Goto { offset } => {
            *offset =
                i8::try_from(delta).map_err(|_| invalid("instruction rewrite", "goto overflow"))?;
        }
        Goto16 { offset }
        | IfEq { offset, .. }
        | IfNe { offset, .. }
        | IfLt { offset, .. }
        | IfGe { offset, .. }
        | IfGt { offset, .. }
        | IfLe { offset, .. }
        | IfEqz { offset, .. }
        | IfNez { offset, .. }
        | IfLtz { offset, .. }
        | IfGez { offset, .. }
        | IfGtz { offset, .. }
        | IfLez { offset, .. } => {
            *offset = i16::try_from(delta)
                .map_err(|_| invalid("instruction rewrite", "conditional overflow"))?;
        }
        Goto32 { offset } => *offset = delta,
        _ => return Err(invalid("instruction rewrite", "expected branch")),
    }
    Ok(())
}
pub(crate) fn invert_condition(insn: &Instruction) -> Result<Instruction> {
    use Instruction::{
        IfEq, IfEqz, IfGe, IfGez, IfGt, IfGtz, IfLe, IfLez, IfLt, IfLtz, IfNe, IfNez,
    };
    Ok(match *insn {
        IfEq { a, b, offset } => IfNe { a, b, offset },
        IfNe { a, b, offset } => IfEq { a, b, offset },
        IfLt { a, b, offset } => IfGe { a, b, offset },
        IfGe { a, b, offset } => IfLt { a, b, offset },
        IfGt { a, b, offset } => IfLe { a, b, offset },
        IfLe { a, b, offset } => IfGt { a, b, offset },
        IfEqz { a, offset } => IfNez { a, offset },
        IfNez { a, offset } => IfEqz { a, offset },
        IfLtz { a, offset } => IfGez { a, offset },
        IfGez { a, offset } => IfLtz { a, offset },
        IfGtz { a, offset } => IfLez { a, offset },
        IfLez { a, offset } => IfGtz { a, offset },
        _ => return Err(invalid("instruction rewrite", "expected condition")),
    })
}
