// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::{self, TestResult};
use reseam_dex::ParseOptions;

#[test]
fn edits_relocate_debug_and_handlers_and_roll_back_invalid_targets() -> TestResult {
    use reseam_dex::DebugBytecode;
    use reseam_dex::{CatchHandler, Instruction, TryItem};
    let mut dex = common::dex();
    let code = common::body(&mut dex);
    code.set_exception_handlers(
        vec![TryItem {
            start_addr: 0,
            insn_count: 2,
            handler_idx: 0,
        }],
        vec![CatchHandler {
            typed_catches: Vec::new(),
            catch_all_addr: Some(2),
        }],
    )?;
    let original = code.clone();
    assert!(
        code.replace_instructions(0, &[Instruction::Goto { offset: 1 }])
            .is_err()
    );
    assert_eq!(*code, original);
    assert!(
        code.edit_instructions(|instructions| {
            instructions[0] = Instruction::Nop;
            reseam_dex::CodeItem::new(0, 1, 0, Vec::new()).map(|_| ())
        })
        .is_err()
    );
    assert_eq!(*code, original);
    code.insert_instructions(
        1,
        &[Instruction::Const {
            dest: 1,
            value: 700,
        }],
    )?;
    assert_eq!(code.tries()[0].insn_count, 5);
    assert_eq!(code.catch_handlers()[0].catch_all_addr, Some(5));
    let debug = code.debug_info().expect("fixture").read()?;
    assert_eq!(debug.line_start, 42);
    let pc: u32 = debug
        .bytecodes
        .iter()
        .map(|event| match event {
            DebugBytecode::AdvancePc { advance } => *advance,
            DebugBytecode::SpecialAdvance { pc_advance, .. } => *pc_advance,
            _ => 0,
        })
        .sum();
    assert_eq!(pc, 5);
    drop(debug);
    code.edit_instructions(|instructions| {
        instructions[1] = Instruction::Const16 {
            dest: 1,
            value: 700,
        };
        Ok(())
    })?;
    assert_eq!(code.tries()[0].insn_count, 4);
    assert_eq!(code.catch_handlers()[0].catch_all_addr, Some(4));
    // ART debug programs may describe a position within an instruction's code units.
    code.set_debug_info(Some(reseam_dex::Metadata::new(reseam_dex::DebugInfo {
        line_start: 42,
        parameter_names: Vec::new(),
        bytecodes: vec![
            DebugBytecode::AdvancePc { advance: 1 },
            DebugBytecode::EndSequence,
        ],
    })));
    code.insert_instructions(0, &[Instruction::Nop])?;
    let bytes = common::write(&dex)?;
    reseam_dex::parse(&bytes, ParseOptions::default())?;
    Ok(())
}

#[test]
fn jumps_name_switch_cases_and_branch_targets_by_instruction_index() -> TestResult {
    use reseam_dex::{CodeItem, Instruction, Jumps, PackedSwitchData, jumps};
    let code = CodeItem::new(
        1,
        1,
        0,
        vec![
            Instruction::PackedSwitch {
                test: 0,
                payload_offset: 10,
            },
            Instruction::Goto { offset: 5 },
            Instruction::Const4 { dest: 0, value: 1 },
            Instruction::ReturnVoid,
            Instruction::IfEqz { a: 0, offset: -2 },
            Instruction::ReturnVoid,
            Instruction::Nop,
            Instruction::PackedSwitchPayload(Box::new(PackedSwitchData {
                first_key: 10,
                targets: vec![4, 6],
            })),
        ],
    )?;
    let jumps_at = |index| jumps(&code, index).expect("control flow is known");
    assert_eq!(
        jumps_at(0),
        Jumps {
            falls_through: Some(1),
            branch: None,
            cases: vec![(10, 2), (11, 4)],
        }
    );
    assert_eq!(
        jumps_at(1),
        Jumps {
            falls_through: None,
            branch: Some(5),
            cases: Vec::new(),
        }
    );
    assert_eq!(
        jumps_at(4),
        Jumps {
            falls_through: Some(5),
            branch: Some(2),
            cases: Vec::new(),
        }
    );
    assert_eq!(jumps_at(3).falls_through, None);
    Ok(())
}
