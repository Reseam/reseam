// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{
    Allocation, DexFile, Instruction, InstructionExpansion, RegisterKind, Result, invalid, kind,
    move_register,
};

pub(super) fn lower_invoke(
    expansion: &mut InstructionExpansion,
    allocation: &mut Allocation<'_>,
    dex: &DexFile,
) -> Result<()> {
    let insn = &expansion.instruction;
    let registers: Vec<u16> = match insn.arguments() {
        crate::types::register_operands::InstructionArguments::List(args) => {
            args.iter().map(|&register| u16::from(register)).collect()
        }
        crate::types::register_operands::InstructionArguments::Range { first, count } => {
            (u32::from(first)..u32::from(first) + u32::from(count))
                .map(|register| {
                    u16::try_from(register)
                        .map_err(|_| invalid("register growth", "invoke range exceeds frame"))
                })
                .collect::<Result<_>>()?
        }
        crate::types::register_operands::InstructionArguments::None => {
            return Err(invalid(
                "register growth",
                "instruction has no invoke arguments",
            ));
        }
    };
    let shifted: Vec<u16> = registers.iter().map(|reg| allocation.shift(*reg)).collect();
    let arguments = invoke_arguments(insn, registers.len(), dex)?;
    if arguments
        .iter()
        .map(|kind| kind.words() as usize)
        .sum::<usize>()
        != registers.len()
    {
        return Err(invalid(
            "register growth",
            "invoke argument count does not match prototype",
        ));
    }
    let mut word = 0;
    for kind in &arguments {
        if *kind == RegisterKind::Wide && shifted[word + 1] != shifted[word] + 1 {
            return Err(invalid(
                "register growth",
                "wide invoke argument crosses the local/parameter boundary",
            ));
        }
        word += kind.words() as usize;
    }
    let encoding = if shifted.len() <= 5 && shifted.iter().all(|reg| *reg <= 15) {
        InvokeEncoding::Compact
    } else {
        InvokeEncoding::Range
    };
    let consecutive = shifted.windows(2).all(|pair| pair[1] == pair[0] + 1);
    let first = if matches!(encoding, InvokeEncoding::Compact) || consecutive {
        shifted.first().copied().unwrap_or(0)
    } else {
        let words = shifted.len() as u16;
        let scratch = match allocation.dead_scratch(words, u16::MAX) {
            Some(register) => register,
            None => allocation.arguments.reserve(words)?,
        };
        let mut word = 0;
        for kind in arguments {
            expansion
                .before
                .push(move_register(scratch + word as u16, shifted[word], kind));
            word += kind.words() as usize;
        }
        scratch
    };
    let count = u8::try_from(shifted.len())
        .map_err(|_| invalid("register growth", "invoke exceeds 255 words"))?;
    expansion.instruction = invoke_encoding(insn, &shifted, first, count, encoding)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum InvokeEncoding {
    Compact,
    Range,
}

fn invoke_encoding(
    insn: &Instruction,
    shifted: &[u16],
    first: u16,
    count: u8,
    encoding: InvokeEncoding,
) -> Result<Instruction> {
    macro_rules! encode {
        ($($compact:ident => $range:ident { $($field:ident),+ }),* $(,)?) => {
            Ok(match *insn {
                $(Instruction::$compact { $($field,)* .. }
                | Instruction::$range { $($field,)* .. } => match encoding {
                    InvokeEncoding::Compact => Instruction::$compact {
                        $($field,)*
                        args: crate::types::instruction::RegList::try_from_iter(
                            shifted.iter().map(|reg| *reg as u8),
                        )?,
                    },
                    InvokeEncoding::Range => Instruction::$range {
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

#[derive(Clone, Copy)]
enum Receiver {
    Absent,
    Present,
}

fn prototype_arguments(
    dex: &DexFile,
    prototype: crate::Prototype,
    receiver: Receiver,
) -> Vec<RegisterKind> {
    let mut arguments = Vec::new();
    if matches!(receiver, Receiver::Present) {
        arguments.push(RegisterKind::Object);
    }
    arguments.extend(
        prototype
            .parameters
            .into_iter()
            .map(|param| kind(&dex.type_descriptor(param))),
    );
    arguments
}

fn invoke_arguments(insn: &Instruction, words: usize, dex: &DexFile) -> Result<Vec<RegisterKind>> {
    use Instruction::{
        FilledNewArray, FilledNewArrayRange, InvokeCustom, InvokeCustomRange, InvokeDirect,
        InvokeDirectRange, InvokeInterface, InvokeInterfaceRange, InvokePolymorphic,
        InvokePolymorphicRange, InvokeStatic, InvokeStaticRange, InvokeSuper, InvokeSuperRange,
        InvokeVirtual, InvokeVirtualRange,
    };
    let arguments = match insn {
        FilledNewArray { type_, .. } | FilledNewArrayRange { type_, .. } => {
            let descriptor = dex.type_descriptor(*type_);
            let element = descriptor.strip_prefix('[').ok_or_else(|| {
                invalid("register growth", "filled-new-array type is not an array")
            })?;
            vec![kind(element); words]
        }
        InvokePolymorphic { proto, .. } | InvokePolymorphicRange { proto, .. } => {
            prototype_arguments(dex, dex.proto(*proto), Receiver::Present)
        }
        InvokeCustom { call_site, .. } | InvokeCustomRange { call_site, .. } => {
            let call_site = dex.call_sites.get(call_site.0 as usize)?;
            prototype_arguments(dex, dex.proto(call_site.method_type), Receiver::Absent)
        }
        InvokeVirtual { method, .. }
        | InvokeVirtualRange { method, .. }
        | InvokeSuper { method, .. }
        | InvokeSuperRange { method, .. }
        | InvokeDirect { method, .. }
        | InvokeDirectRange { method, .. }
        | InvokeInterface { method, .. }
        | InvokeInterfaceRange { method, .. } => prototype_arguments(
            dex,
            dex.proto(dex.method_id(*method).proto),
            Receiver::Present,
        ),
        InvokeStatic { method, .. } | InvokeStaticRange { method, .. } => prototype_arguments(
            dex,
            dex.proto(dex.method_id(*method).proto),
            Receiver::Absent,
        ),
        _ => return Err(invalid("register growth", "instruction is not an invoke")),
    };
    Ok(arguments)
}
