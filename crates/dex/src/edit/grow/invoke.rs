// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::allocation::Allocation;
use super::widen::move_register;
use crate::DexFile;
use crate::edit::invoke::{
    Invocation, InvokeEncoding, InvokeForm, Receiver, argument_kinds, array_kinds,
};
use crate::edit::relocate::InstructionExpansion;
use crate::error::{Result, invalid};
use crate::instruction::Instruction;
use crate::instruction::operands::{InstructionArguments, RegisterKind};

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

fn invoke_encoding(
    insn: &Instruction,
    shifted: &[u16],
    encoding: InvokeEncoding,
) -> Result<Instruction> {
    macro_rules! encode {
        ($($compact:ident => $range:ident { $($field:ident),+ }),* $(,)?) => {
            Ok(match *insn {
                $(Instruction::$compact { $($field,)* .. }
                | Instruction::$range { $($field,)* .. } => match encoding {
                    InvokeEncoding::Compact => Instruction::$compact {
                        $($field,)*
                        args: crate::instruction::RegList::try_from_iter(
                            shifted.iter().map(|reg| *reg as u8),
                        )?,
                    },
                    InvokeEncoding::Range { first, count } => Instruction::$range {
                        $($field,)*
                        first_reg: first,
                        count,
                    },
                },)*
                _ => return Err(invalid("register growth", "instruction is not an invoke")),
            })
        };
    }
    encode!(
        FilledNewArray => FilledNewArrayRange { type_ },
        InvokeVirtual => InvokeVirtualRange { method },
        InvokeSuper => InvokeSuperRange { method },
        InvokeDirect => InvokeDirectRange { method },
        InvokeStatic => InvokeStaticRange { method },
        InvokeInterface => InvokeInterfaceRange { method },
        InvokePolymorphic => InvokePolymorphicRange { method, proto },
        InvokeCustom => InvokeCustomRange { call_site },
    )
}

fn invoke_kinds(insn: &Instruction, words: usize, dex: &DexFile) -> Result<Vec<RegisterKind>> {
    use Instruction::{
        FilledNewArray, FilledNewArrayRange, InvokeCustom, InvokeCustomRange, InvokeDirect,
        InvokeDirectRange, InvokeInterface, InvokeInterfaceRange, InvokePolymorphic,
        InvokePolymorphicRange, InvokeStatic, InvokeStaticRange, InvokeSuper, InvokeSuperRange,
        InvokeVirtual, InvokeVirtualRange,
    };
    let prototype =
        |proto, receiver| argument_kinds(&dex.proto_descriptor(&dex.proto(proto)), receiver);
    match insn {
        FilledNewArray { type_, .. } | FilledNewArrayRange { type_, .. } => {
            array_kinds(&dex.type_descriptor(*type_), words)
        }
        InvokePolymorphic { proto, .. } | InvokePolymorphicRange { proto, .. } => {
            prototype(*proto, Receiver::Present)
        }
        InvokeCustom { call_site, .. } | InvokeCustomRange { call_site, .. } => {
            prototype(dex.call_site(*call_site)?.method_type, Receiver::Absent)
        }
        InvokeVirtual { method, .. }
        | InvokeVirtualRange { method, .. }
        | InvokeSuper { method, .. }
        | InvokeSuperRange { method, .. }
        | InvokeDirect { method, .. }
        | InvokeDirectRange { method, .. }
        | InvokeInterface { method, .. }
        | InvokeInterfaceRange { method, .. } => {
            prototype(dex.method_id(*method).proto, Receiver::Present)
        }
        InvokeStatic { method, .. } | InvokeStaticRange { method, .. } => {
            prototype(dex.method_id(*method).proto, Receiver::Absent)
        }
        _ => Err(invalid("register growth", "instruction is not an invoke")),
    }
}
