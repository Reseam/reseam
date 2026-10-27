// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

#[test]
fn payload_references_survive_alignment_during_assembly() {
    use crate::bridge::dex::types::{BranchInsn, FillArrayInsn};
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
        let (directory, mut apk) = crate::test_support::apk();
        apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
        let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
        let _run = RunGuard::enter().unwrap();
        let guard =
            ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned()))
                .unwrap();
        let class = fixture_class(0, "LPayloads;");
        let method = methods::add_method(
            class,
            NewMethod {
                instructions,
                tries: vec![],
                catch_handlers: vec![],
                ..fixture_method("fill".into(), "([B)V".into(), 1, 1)
            },
        );
        let location = crate::bridge::handles::method_location(method).unwrap();
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
fn assembling_keeps_branches_over_calls_it_reencodes() {
    use crate::bridge::dex::types::{Branch0Insn, InvokeInsn, MethodRef};
    use reseam_dex::Instruction as NativeInstruction;

    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
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
        let instructions = vec![
            branch,
            Instruction::Invoke(InvokeInsn {
                opcode: 0x71,
                registers: vec![20, 21],
                method: MethodRef {
                    defining_class: "LObserver;".into(),
                    name: "observe".into(),
                    proto: "(II)V".into(),
                },
            }),
            Instruction::Simple(SimpleInsn { opcode: 0x0e }),
        ];
        let method = methods::add_method(
            class,
            NewMethod {
                outs_size: 2,
                instructions,
                ..fixture_method(format!("run{index}"), "()V".into(), 31, 0)
            },
        );
        locations.push(crate::bridge::handles::method_location(method).unwrap());
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
