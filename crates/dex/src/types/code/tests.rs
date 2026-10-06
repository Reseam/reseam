// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{CatchHandler, CodeItem, Instruction, TryItem};

enum Edit {
    Insert(usize),
    Remove(usize),
}

fn code_item(instructions: Vec<Instruction>) -> CodeItem {
    CodeItem {
        registers_size: 0,
        ins_size: 0,
        outs_size: 0,
        debug_info: None,
        instructions,
        tries: Vec::<TryItem>::new(),
        catch_handlers: Vec::<CatchHandler>::new(),
    }
}

fn packed_switch_method() -> CodeItem {
    CodeItem {
        registers_size: 1,
        ins_size: 0,
        outs_size: 0,
        debug_info: None,
        instructions: vec![
            Instruction::PackedSwitch {
                test: 0,
                payload_offset: 8,
            },
            Instruction::Nop,
            Instruction::ReturnVoid,
            Instruction::Nop,
            Instruction::ReturnVoid,
            Instruction::Nop,
            Instruction::PackedSwitchPayload(Box::new(
                crate::types::instruction::PackedSwitchData {
                    first_key: 0,
                    targets: vec![3, 5],
                },
            )),
        ],
        tries: Vec::new(),
        catch_handlers: Vec::new(),
    }
}

fn payload_offset(code: &CodeItem) -> i32 {
    match &code.instructions[0] {
        Instruction::PackedSwitch { payload_offset, .. }
        | Instruction::SparseSwitch { payload_offset, .. } => *payload_offset,
        other => panic!("expected switch, got {other:?}"),
    }
}

#[test]
fn switch_edits_relocate_targets_and_keep_payloads_aligned() {
    for sparse in [false, true] {
        for (edit, (offset, targets)) in [
            (Edit::Insert(1), (10, vec![5, 7])),
            (Edit::Insert(3), (10, vec![3, 7])),
            (Edit::Remove(2), (8, vec![3, 5])),
            (Edit::Remove(5), (8, vec![3, 5])),
        ] {
            let mut code = packed_switch_method();
            if sparse {
                code.instructions[0] = Instruction::SparseSwitch {
                    test: 0,
                    payload_offset: 8,
                };
                *code.instructions.last_mut().expect("payload") =
                    Instruction::SparseSwitchPayload(Box::new(crate::SparseSwitchData {
                        keys_and_targets: vec![(10, 3), (20, 5)],
                    }));
            }
            match edit {
                Edit::Insert(at) => {
                    code.insert_instructions(at, &[Instruction::Nop, Instruction::Nop])
                        .expect("insert");
                }
                Edit::Remove(at) => {
                    code.remove_instruction(at).expect("remove");
                }
            }
            assert_eq!(payload_offset(&code), offset);
            let actual = match code.instructions.last().expect("payload") {
                Instruction::PackedSwitchPayload(data) => data.targets.clone(),
                Instruction::SparseSwitchPayload(data) => data
                    .keys_and_targets
                    .iter()
                    .map(|&(_, target)| target)
                    .collect(),
                _ => panic!("switch payload"),
            };
            assert_eq!(actual, targets);
        }
    }
}

#[test]
fn branch_edits_keep_targets_on_surviving_instructions() {
    use Instruction::*;
    for inserted in 1..6 {
        for (instructions, (at, expected)) in [
            (
                vec![Nop, Nop, Goto { offset: -2 }],
                (1, vec![-2 - inserted as i32]),
            ),
            (
                vec![
                    Goto { offset: 3 },
                    Nop,
                    Goto { offset: 2 },
                    Nop,
                    Goto { offset: -2 },
                ],
                (
                    3,
                    vec![
                        3 + inserted as i32,
                        2 + inserted as i32,
                        -2 - inserted as i32,
                    ],
                ),
            ),
        ] {
            let mut code = code_item(instructions);
            code.insert_instructions(at, &vec![Nop; inserted])
                .expect("insert");
            let offsets: Vec<_> = code
                .instructions()
                .iter()
                .filter_map(|instruction| match instruction {
                    Goto { offset } => Some(i32::from(*offset)),
                    _ => None,
                })
                .collect();
            assert_eq!(offsets, expected);
        }
    }
    let mut code = code_item(vec![Goto { offset: 2 }, Nop, Nop, ReturnVoid]);
    code.remove_instruction(2).expect("remove target");
    assert_eq!(code.instructions(), [Goto { offset: 2 }, Nop, ReturnVoid]);
}
