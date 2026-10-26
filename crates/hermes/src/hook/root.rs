// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use crate::FunctionId;
use crate::HermesFile;
use crate::assemble::assemble;
use crate::closure::environment_depth;
use crate::edit::{EditedFunction, Edits};
use crate::error::Result;
use crate::function::Function;
use crate::opcode::{Instruction, Op};

/// The app functions that hold the linked modules' private environment, rewritten to carry it
/// to the closures that need it.
pub(crate) fn rooted_functions(
    file: &HermesFile<'_>,
    edits: &Edits,
) -> Result<BTreeMap<FunctionId, EditedFunction>> {
    edits
        .roots()
        .iter()
        .map(|(&id, root)| {
            let function = file.function(id)?;
            let body = attach_root(&function, function.instructions()?, root.depth, &root.sites)?;
            // A hooked function's original body lives on in the function its hook calls.
            Ok((edits.relocated(id).unwrap_or(id), body))
        })
        .collect()
}

fn attach_root(
    function: &Function<'_>,
    code: Vec<Instruction>,
    depth: u16,
    attach: &BTreeSet<usize>,
) -> Result<EditedFunction> {
    let depth = environment_depth(function.id, depth)?;
    let mut rewritten = vec![Instruction::new(
        Op::GetParentEnvironment,
        &[0, u64::from(depth)],
    )];
    for (index, mut inst) in code.into_iter().enumerate() {
        for (operand, value) in inst.definition().operands.iter().zip(&mut inst.values) {
            if operand.kind.is_register() {
                *value += 1;
            }
        }
        if attach.contains(&index) {
            inst.values[1] = 0;
        }
        if inst.op == Op::CreateTopLevelEnvironment {
            let mut replacement =
                Instruction::new(Op::CreateEnvironment, &[inst.values[0], 0, inst.values[1]]);
            replacement.offset = inst.offset;
            rewritten.push(replacement);
        } else {
            rewritten.push(inst);
        }
    }
    let mut function = assemble(function, rewritten, None)?;
    function.header.frame_size += 1;
    function.header.number_regs = 0;
    function.header.non_pointer_regs = 0;
    Ok(function)
}
