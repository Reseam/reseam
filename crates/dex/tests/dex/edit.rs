// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
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
