// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;
use reseam_apk::reseam_dex::{self as dex, util::descriptor::parse_method_descriptor};

use super::types::{
    CustomRangeInsn, FilledArrayRangeInsn, Instruction, InvokeRangeInsn, PolymorphicRangeInsn,
    Reg2Insn,
};

#[derive(Clone, Copy)]
enum ArgumentKind {
    Value,
    Object,
    Wide,
}

impl ArgumentKind {
    fn words(self) -> usize {
        match self {
            Self::Wide => 2,
            Self::Value | Self::Object => 1,
        }
    }

    fn descriptor(value: &str) -> Self {
        match value.as_bytes().first() {
            Some(b'J' | b'D') => Self::Wide,
            Some(b'L' | b'[') => Self::Object,
            _ => Self::Value,
        }
    }
}

struct Invocation {
    registers: Vec<u16>,
    arguments: Vec<ArgumentKind>,
    encoding: Encoding,
}

#[derive(Clone, Copy)]
enum Encoding {
    Compact,
    Range,
}

fn invalid(reason: impl Into<String>) -> dex::DexError {
    dex::DexError::Invalid {
        section: "invoke",
        reason: reason.into(),
    }
}

fn parameters(proto: &str) -> dex::Result<Vec<ArgumentKind>> {
    let (parameters, _) = parse_method_descriptor(proto)
        .ok_or_else(|| invalid(format!("invalid prototype {proto}")))?;
    Ok(parameters
        .into_iter()
        .map(ArgumentKind::descriptor)
        .collect())
}

pub(super) fn incoming_words(proto: &str, flags: dex::AccessFlags) -> dex::Result<u16> {
    let receiver = if flags.contains(dex::AccessFlags::STATIC) {
        Receiver::Absent
    } else {
        Receiver::Present
    };
    let words: usize = method_arguments(proto, receiver)?
        .iter()
        .map(|argument| argument.words())
        .sum();
    u16::try_from(words).map_err(|_| invalid("incoming register words exceed 65535"))
}

fn invocation(instruction: &Instruction) -> dex::Result<Option<Invocation>> {
    let value = match instruction {
        Instruction::Invoke(value) if (0x6e..=0x72).contains(&value.opcode) => Invocation {
            registers: value.registers.clone(),
            arguments: method_arguments(
                &value.method.proto,
                if value.opcode == 0x71 {
                    Receiver::Absent
                } else {
                    Receiver::Present
                },
            )?,
            encoding: Encoding::Compact,
        },
        Instruction::Polymorphic(value) if value.opcode == 0xfa => Invocation {
            registers: value.registers.clone(),
            arguments: method_arguments(&value.proto, Receiver::Present)?,
            encoding: Encoding::Compact,
        },
        Instruction::Custom(value) if value.opcode == 0xfc => Invocation {
            registers: value.registers.clone(),
            arguments: parameters(&value.call_site.proto)?,
            encoding: Encoding::Compact,
        },
        Instruction::FilledArray(value) if value.opcode == 0x24 => Invocation {
            registers: value.registers.clone(),
            arguments: array_arguments(&value.type_descriptor, value.registers.len())?,
            encoding: Encoding::Compact,
        },
        _ => return ranged_invocation(instruction),
    };
    value.validate()?;
    Ok(Some(value))
}

#[derive(Clone, Copy)]
enum Receiver {
    Present,
    Absent,
}

fn method_arguments(proto: &str, receiver: Receiver) -> dex::Result<Vec<ArgumentKind>> {
    let mut arguments = parameters(proto)?;
    if matches!(receiver, Receiver::Present) {
        arguments.insert(0, ArgumentKind::Object);
    }
    Ok(arguments)
}

fn ranged_invocation(instruction: &Instruction) -> dex::Result<Option<Invocation>> {
    let (registers, arguments) = match instruction {
        Instruction::InvokeRange(value) if (0x74..=0x78).contains(&value.opcode) => (
            range_registers(value.start_reg, value.reg_count)?,
            method_arguments(
                &value.method.proto,
                if value.opcode == 0x77 {
                    Receiver::Absent
                } else {
                    Receiver::Present
                },
            )?,
        ),
        Instruction::PolymorphicRange(value) if value.opcode == 0xfb => (
            range_registers(value.start_reg, value.reg_count)?,
            method_arguments(&value.proto, Receiver::Present)?,
        ),
        Instruction::CustomRange(value) if value.opcode == 0xfd => (
            range_registers(value.start_reg, value.reg_count)?,
            parameters(&value.call_site.proto)?,
        ),
        Instruction::FilledArrayRange(value) if value.opcode == 0x25 => (
            range_registers(value.start_reg, value.reg_count)?,
            array_arguments(&value.type_descriptor, usize::from(value.reg_count))?,
        ),
        Instruction::Invoke(_)
        | Instruction::InvokeRange(_)
        | Instruction::Polymorphic(_)
        | Instruction::PolymorphicRange(_)
        | Instruction::Custom(_)
        | Instruction::CustomRange(_)
        | Instruction::FilledArray(_)
        | Instruction::FilledArrayRange(_) => {
            return Err(invalid("opcode does not match invocation shape"));
        }
        _ => return Ok(None),
    };
    let value = Invocation {
        registers,
        arguments,
        encoding: Encoding::Range,
    };
    value.validate()?;
    Ok(Some(value))
}

fn array_arguments(descriptor: &str, count: usize) -> dex::Result<Vec<ArgumentKind>> {
    let element = descriptor
        .strip_prefix('[')
        .filter(|_| dex::util::descriptor::is_type_descriptor(descriptor))
        .ok_or_else(|| invalid("filled-new-array requires an array descriptor"))?;
    let kind = ArgumentKind::descriptor(element);
    if matches!(kind, ArgumentKind::Wide) {
        return Err(invalid("filled-new-array cannot contain wide primitives"));
    }
    Ok(vec![kind; count])
}

fn range_registers(start: u16, count: u16) -> dex::Result<Vec<u16>> {
    u8::try_from(count).map_err(|_| invalid("range exceeds 255 register words"))?;
    (u32::from(start)..u32::from(start) + u32::from(count))
        .map(|register| u16::try_from(register).map_err(|_| invalid("range exceeds v65535")))
        .collect()
}

impl Invocation {
    fn validate(&self) -> dex::Result<()> {
        u8::try_from(self.registers.len())
            .map_err(|_| invalid("invocation exceeds 255 register words"))?;
        if self
            .arguments
            .iter()
            .map(|kind| kind.words())
            .sum::<usize>()
            != self.registers.len()
        {
            return Err(invalid("register word count does not match prototype"));
        }
        let mut word = 0;
        for kind in &self.arguments {
            if matches!(kind, ArgumentKind::Wide)
                && self.registers[word].checked_add(1) != Some(self.registers[word + 1])
            {
                return Err(invalid("wide argument must occupy consecutive registers"));
            }
            word += kind.words();
        }
        Ok(())
    }

    fn compact(&self) -> bool {
        matches!(self.encoding, Encoding::Compact)
            && self.registers.len() <= 5
            && self.registers.iter().all(|reg| *reg <= 15)
    }

    fn scratch_words(&self) -> usize {
        if self.compact() || consecutive(&self.registers) {
            0
        } else {
            self.registers.len()
        }
    }

    fn lower(self, scratch: &[u16]) -> dex::Result<Option<Lowering>> {
        if self.compact() || matches!(self.encoding, Encoding::Range) {
            return Ok(None);
        }
        let mut instructions = Vec::with_capacity(self.arguments.len() + 1);
        let first = if self.scratch_words() == 0 {
            self.registers.first().copied().unwrap_or(0)
        } else {
            if scratch.len() != self.registers.len()
                || !consecutive(scratch)
                || scratch
                    .iter()
                    .any(|register| self.registers.contains(register))
            {
                return Err(invalid(
                    "scratch span must be consecutive, match the argument words and exclude source registers",
                ));
            }
            let mut word = 0;
            for kind in self.arguments {
                instructions.push(move_argument(scratch[word], self.registers[word], kind));
                word += kind.words();
            }
            scratch[0]
        };
        let count = self.registers.len() as u16;
        Ok(Some(Lowering {
            moves: instructions,
            first,
            count,
        }))
    }
}

fn consecutive(registers: &[u16]) -> bool {
    registers
        .windows(2)
        .all(|pair| pair[0].checked_add(1) == Some(pair[1]))
}

fn move_argument(dest: u16, src: u16, kind: ArgumentKind) -> Instruction {
    let base = match kind {
        ArgumentKind::Value => 1,
        ArgumentKind::Wide => 4,
        ArgumentKind::Object => 7,
    };
    let opcode = base
        + if dest <= 15 && src <= 15 {
            0
        } else if dest <= 255 {
            1
        } else {
            2
        };
    Instruction::Reg2(Reg2Insn {
        opcode,
        reg_a: dest,
        reg_b: src,
    })
}

struct Lowering {
    moves: Vec<Instruction>,
    first: u16,
    count: u16,
}

fn range(instruction: Instruction, first: u16, count: u16) -> dex::Result<Instruction> {
    Ok(match instruction {
        Instruction::Invoke(value) => Instruction::InvokeRange(InvokeRangeInsn {
            opcode: value.opcode + 6,
            start_reg: first,
            reg_count: count,
            method: value.method,
        }),
        Instruction::Polymorphic(value) => Instruction::PolymorphicRange(PolymorphicRangeInsn {
            opcode: 0xfb,
            start_reg: first,
            reg_count: count,
            method: value.method,
            proto: value.proto,
        }),
        Instruction::Custom(value) => Instruction::CustomRange(CustomRangeInsn {
            opcode: 0xfd,
            start_reg: first,
            reg_count: count,
            call_site: value.call_site,
        }),
        Instruction::FilledArray(value) => Instruction::FilledArrayRange(FilledArrayRangeInsn {
            opcode: 0x25,
            start_reg: first,
            reg_count: count,
            type_descriptor: value.type_descriptor,
        }),
        _ => return Err(invalid("range promotion requires a compact invocation")),
    })
}

pub(super) fn lower(
    instruction: &Instruction,
    scratch: &[u16],
) -> dex::Result<Option<Vec<Instruction>>> {
    let plan = invocation(instruction)?
        .map(|invoke| invoke.lower(scratch))
        .transpose()?
        .flatten();
    plan.map(|mut plan| {
        plan.moves
            .push(range(instruction.clone(), plan.first, plan.count)?);
        Ok(plan.moves)
    })
    .transpose()
}

/// Scratch words each instruction needs to be lowered; zero for all but wide invokes.
#[export]
pub fn invoke_scratch_words(instructions: Vec<Instruction>) -> Result<Vec<u32>, String> {
    instructions
        .iter()
        .map(|instruction| scratch_words(instruction).map(|words| words as u32))
        .collect::<dex::Result<_>>()
        .map_err(|error| error.to_string())
}

pub(super) fn scratch_words(instruction: &Instruction) -> dex::Result<usize> {
    invocation(instruction).map(|invoke| invoke.map_or(0, |value| value.scratch_words()))
}

#[export]
pub fn lower_instruction(
    instruction: Instruction,
    scratch: Vec<u16>,
) -> Result<Vec<Instruction>, String> {
    let plan = invocation(&instruction)
        .and_then(|value| value.map(|invoke| invoke.lower(&scratch)).transpose())
        .map_err(|error| error.to_string())?
        .flatten();
    Ok(match plan {
        None => vec![instruction],
        Some(mut plan) => {
            plan.moves.push(
                range(instruction, plan.first, plan.count).map_err(|error| error.to_string())?,
            );
            plan.moves
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{invoke_scratch_words, lower_instruction};
    use crate::kotlin::types::{Instruction, InvokeInsn, MethodRef};

    #[test]
    fn invoke_encodings_preserve_typed_argument_words() {
        struct Case {
            registers: Vec<u16>,
            scratch: Vec<u16>,
        }
        for case in [
            Case {
                registers: vec![1, 2, 3, 4],
                scratch: vec![],
            },
            Case {
                registers: vec![20, 21, 22, 23],
                scratch: vec![],
            },
            Case {
                registers: vec![20, 16, 21, 22],
                scratch: vec![0, 1, 2, 3],
            },
        ] {
            let input = Instruction::Invoke(InvokeInsn {
                opcode: 0x71,
                registers: case.registers.clone(),
                method: MethodRef {
                    defining_class: "LObserver;".into(),
                    name: "observe".into(),
                    proto: "(ILjava/lang/Object;J)V".into(),
                },
            });
            assert_eq!(
                invoke_scratch_words(vec![input.clone()]).unwrap(),
                [case.scratch.len() as u32]
            );
            let lowered = lower_instruction(input, case.scratch).unwrap();
            let mut values: BTreeMap<u16, u16> =
                case.registers.iter().map(|reg| (*reg, *reg)).collect();
            for (argument, instruction) in lowered[..lowered.len() - 1].iter().enumerate() {
                let Instruction::Reg2(move_) = instruction else {
                    panic!("argument staging must copy values")
                };
                let expected = [1..=3, 7..=9, 4..=6];
                assert!(
                    expected[argument].contains(&move_.opcode),
                    "move encoding must preserve the argument type"
                );
                values.insert(move_.reg_a, values[&move_.reg_b]);
                if (4..=6).contains(&move_.opcode) {
                    values.insert(move_.reg_a + 1, values[&(move_.reg_b + 1)]);
                }
            }
            let arguments: Vec<u16> = match lowered.last().unwrap() {
                Instruction::Invoke(call) => call.registers.clone(),
                Instruction::InvokeRange(call) => {
                    (call.start_reg..call.start_reg + call.reg_count).collect()
                }
                _ => panic!("lowering must end with the call"),
            };
            assert_eq!(
                arguments.iter().map(|reg| values[reg]).collect::<Vec<_>>(),
                case.registers
            );
        }
        for registers in [vec![0, 1], vec![0, 2, 3], vec![0, 1, 3, 5]] {
            assert!(
                lower_instruction(
                    Instruction::Invoke(InvokeInsn {
                        opcode: 0x71,
                        registers,
                        method: MethodRef {
                            defining_class: "LObserver;".into(),
                            name: "observe".into(),
                            proto: "(ILjava/lang/Object;J)V".into(),
                        },
                    }),
                    Vec::new()
                )
                .is_err()
            );
        }
    }
}
