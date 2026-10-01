// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

fn code(instructions: Vec<Instruction>) -> CodeItem {
    CodeItem {
        registers_size: 2,
        ins_size: 0,
        outs_size: 0,
        debug_info: None,
        instructions,
        tries: vec![],
        catch_handlers: vec![],
    }
}

#[test]
fn expanded_switch_targets_run_the_prefix_and_payloads_stay_aligned() {
    use Instruction::*;
    let mut code = code(vec![
        PackedSwitch {
            test: 0,
            payload_offset: 4,
        },
        ReturnVoid,
        PackedSwitchPayload(Box::new(crate::PackedSwitchData {
            first_key: 0,
            targets: vec![3],
        })),
    ]);
    let mut expansions: Vec<InstructionExpansion> =
        code.instructions.iter().cloned().map(Into::into).collect();
    expansions[1].before.push(Const4 { dest: 1, value: 0 });
    let indices = code
        .rewrite_instructions(expansions, &BTreeMap::new())
        .unwrap();
    assert_eq!(indices.starts(), [0, 1, 4, 5]);
    assert_eq!(
        code.instructions,
        [
            PackedSwitch {
                test: 0,
                payload_offset: 6
            },
            Const4 { dest: 1, value: 0 },
            ReturnVoid,
            Nop,
            PackedSwitchPayload(Box::new(crate::PackedSwitchData {
                first_key: 0,
                targets: vec![3]
            }))
        ]
    );
}

#[test]
fn widens_forward_conditions_and_backward_gotos_after_expansion() {
    use Instruction::*;
    let mut conditional = code(vec![IfEqz { a: 0, offset: 3 }, Nop, ReturnVoid]);
    let mut expansions: Vec<InstructionExpansion> = conditional
        .instructions
        .iter()
        .cloned()
        .map(Into::into)
        .collect();
    expansions[1].before = vec![Nop; 33_000];
    let indices = conditional
        .rewrite_instructions(expansions, &BTreeMap::new())
        .unwrap();
    assert_eq!(conditional.instructions[0], IfNez { a: 0, offset: 5 });
    assert_eq!(conditional.instructions[1], Goto32 { offset: 33_004 });
    assert_eq!(indices.starts()[2], 33_003);

    let mut backward = code(vec![Nop, Goto { offset: -1 }]);
    let mut expansions: Vec<InstructionExpansion> = backward
        .instructions
        .iter()
        .cloned()
        .map(Into::into)
        .collect();
    expansions[0].before = vec![Nop; 300];
    let indices = backward
        .rewrite_instructions(expansions, &BTreeMap::new())
        .unwrap();
    assert_eq!(
        backward.instructions[indices.starts()[1]],
        Goto16 { offset: -301 }
    );
}
