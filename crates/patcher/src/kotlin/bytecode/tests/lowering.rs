// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

#[test]
fn payload_references_survive_alignment_during_pure_lowering() {
    use crate::kotlin::types::{BranchInsn, FillArrayInsn};
    for prefix in 0..4 {
        let mut instructions = vec![Instruction::Simple(SimpleInsn { opcode: 0 }); prefix];
        instructions.extend([
            Instruction::Branch(BranchInsn {
                opcode: 0x26,
                reg_a: 0,
                offset: 4,
            }),
            Instruction::Simple(SimpleInsn { opcode: 0x0e }),
            Instruction::FillArrayData(FillArrayInsn {
                element_width: 1,
                data: vec![7],
            }),
        ]);
        let lowered = assembly::lower_instructions(instructions, vec![]).unwrap();
        let (directory, mut apk) = crate::test_support::apk();
        apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
        let mut context = PatchContext::new(&mut apk);
        let _run = RunGuard::enter().unwrap();
        let guard =
            ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned()))
                .unwrap();
        let class = fixture_class(0, "LPayloads;");
        let method = methods::add_method(
            class,
            NewMethod {
                instructions: lowered,
                tries: vec![],
                catch_handlers: vec![],
                ..fixture_method("fill".into(), "([B)V".into(), 1, 1)
            },
        );
        let location = crate::kotlin::handles::method_location(method).unwrap();
        guard.finish().unwrap();
        let (_, method) = context.read_method(location).unwrap().unwrap();
        let code = method.code.as_ref().unwrap();
        let addresses: Vec<_> = code
            .instructions()
            .iter()
            .scan(0, |address, instruction| {
                let current = *address;
                *address += instruction.code_units();
                Some((current, instruction))
            })
            .collect();
        let destination = addresses
            .iter()
            .find_map(|(address, instruction)| match instruction {
                reseam_dex::Instruction::FillArrayData { payload_offset, .. } => {
                    address.checked_add_signed(*payload_offset)
                }
                _ => None,
            })
            .unwrap();
        let target = addresses
            .iter()
            .find(|(address, _)| *address == destination)
            .unwrap()
            .1;
        let reseam_dex::Instruction::FillArrayDataPayload(payload) = target else {
            panic!("array initializer targets something other than its data")
        };
        assert_eq!(&payload.data[..], &[7]);
    }
}

#[test]
fn assembling_blocks_keeps_branches_over_lowered_invocations() {
    use crate::kotlin::types::{Branch0Insn, InvokeInsn, MethodRef, ScratchSpan};
    use reseam_dex::Instruction as NativeInstruction;

    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard =
        ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned())).unwrap();
    let class = fixture_class(0, "LAssembly;");
    let mut locations = Vec::new();
    for (index, branch) in [
        Instruction::Branch0(Branch0Insn {
            opcode: 0x28,
            offset: 4,
        }),
        Instruction::Raw(vec![0x28, 0x04]),
    ]
    .into_iter()
    .enumerate()
    {
        let instructions = assembly::lower_instructions(
            vec![
                branch,
                Instruction::Invoke(InvokeInsn {
                    opcode: 0x71,
                    registers: vec![20, 30],
                    method: MethodRef {
                        defining_class: "LObserver;".into(),
                        name: "observe".into(),
                        proto: "(II)V".into(),
                    },
                }),
                Instruction::Simple(SimpleInsn { opcode: 0x0e }),
            ],
            vec![ScratchSpan {
                instruction_index: 1,
                registers: vec![0, 1],
            }],
        )
        .unwrap();
        let method = methods::add_method(
            class,
            NewMethod {
                outs_size: 2,
                instructions,
                ..fixture_method(format!("run{index}"), "()V".into(), 31, 0)
            },
        );
        locations.push(crate::kotlin::handles::method_location(method).unwrap());
    }
    guard.finish().unwrap();
    for location in locations {
        let (_, method) = context.read_method(location).unwrap().unwrap();
        let code = method.code.as_ref().unwrap();
        let NativeInstruction::Goto { offset } = code.instructions()[0] else {
            panic!("branch was lost");
        };
        let return_address: u32 = code.instructions()[..code.instructions().len() - 1]
            .iter()
            .map(NativeInstruction::code_units)
            .sum();
        assert_eq!(u32::try_from(offset).unwrap(), return_address);
        assert!(matches!(
            code.instructions().last(),
            Some(NativeInstruction::ReturnVoid)
        ));
        assert!(
            code.instructions().iter().any(|instruction| matches!(
                instruction,
                NativeInstruction::InvokeStaticRange { .. }
            ))
        );
    }
}

#[test]
fn invoke_encodings_preserve_typed_argument_words() {
    use crate::kotlin::types::{InvokeInsn, MethodRef};
    use std::collections::BTreeMap;
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
            assembly::invoke_scratch_words(vec![input.clone()]).unwrap(),
            [case.scratch.len() as u32]
        );
        let lowered = assembly::lower_instruction(input, case.scratch).unwrap();
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
            assembly::lower_instruction(
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
