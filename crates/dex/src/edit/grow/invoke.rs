// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::allocation::Allocation;
use crate::DexFile;
use crate::edit::forms::move_register;
use crate::edit::invoke::{Invocation, InvokeForm, invoke_encoding, invoke_kinds};
use crate::edit::relocate::InstructionExpansion;
use crate::error::{Result, invalid};
use crate::instruction::operands::InstructionArguments;

/// Re-encodes an invoke or filled-new-array for its shifted arguments,
/// staging them through scratch registers when they no longer form a range.
pub(super) fn lower_invoke(
    expansion: &mut InstructionExpansion,
    allocation: &mut Allocation<'_>,
    dex: &DexFile,
) -> Result<()> {
    let insn = &expansion.instruction;
    let (form, registers): (_, Vec<u16>) = match insn.arguments() {
        InstructionArguments::List(args) => (
            InvokeForm::Compact,
            args.iter()
                .map(|&register| allocation.shift(u16::from(register)))
                .collect(),
        ),
        InstructionArguments::Range { first, count } => (
            InvokeForm::Range,
            (u32::from(first)..u32::from(first) + u32::from(count))
                .map(|register| {
                    u16::try_from(register)
                        .map(|register| allocation.shift(register))
                        .map_err(|_| invalid("register growth", "invoke range exceeds frame"))
                })
                .collect::<Result<_>>()?,
        ),
        InstructionArguments::None => {
            return Err(invalid(
                "register growth",
                "instruction has no invoke arguments",
            ));
        }
    };
    let kinds = invoke_kinds(insn, registers.len(), dex)?;
    let invocation = Invocation::new(form, &registers, &kinds)?;
    let scratch = match invocation.scratch_words() {
        0 => None,
        words => Some(match allocation.dead_scratch(words, u16::MAX) {
            Some(register) => register,
            None => allocation.arguments.reserve(words)?,
        }),
    };
    let (moves, encoding) = invocation.lower(scratch)?;
    expansion.before.extend(
        moves
            .iter()
            .map(|argument| move_register(argument.dest, argument.src, argument.kind)),
    );
    expansion.instruction = invoke_encoding(insn, &registers, encoding)?;
    Ok(())
}
