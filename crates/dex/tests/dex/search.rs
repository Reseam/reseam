// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::{self, TestResult};
use reseam_dex::Loading;

#[test]
fn fingerprint_opcode_windows_agree_for_deferred_and_resident_methods() -> TestResult {
    use reseam_dex::{InstructionPattern, OpcodeMatcher};
    let bytes = common::write(&common::dex())?;
    for loading in [Loading::Deferred, Loading::Eager] {
        let dex = common::parse_with_loading(&bytes, loading)?;
        let reports = |opcodes: &[OpcodeMatcher]| -> reseam_dex::Result<bool> {
            let pattern: Vec<_> = opcodes
                .iter()
                .map(|&opcode| InstructionPattern::Opcode(opcode))
                .collect();
            Ok(dex
                .find_methods_with_opcodes(&pattern)?
                .iter()
                .any(|hit| dex.strings().get(dex.method_id(hit.method).name) == "report"))
        };
        assert!(reports(&[
            OpcodeMatcher::ConstString,
            OpcodeMatcher::ReturnVoid
        ])?);
        assert!(!reports(&[OpcodeMatcher::ReturnObject])?);
    }
    Ok(())
}

#[test]
fn cached_reference_search_tracks_resident_edits_and_class_removal() -> TestResult {
    use reseam_dex::{AccessFlags, CodeItem, EncodedMethod, Instruction};
    let mut dex = common::dex();
    let class = dex.create_class(
        "Lexample/Dashboard;",
        AccessFlags::PUBLIC,
        Some("Ljava/lang/Object;"),
    )?;
    let string = dex.intern_string("dashboard opened");
    let method = dex.intern_method("Lexample/Dashboard;", "report", "()V")?;
    dex.class_mut(class)?.add_method(EncodedMethod {
        method,
        access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
        code: Some(std::sync::Arc::new(CodeItem::new(
            1,
            0,
            0,
            vec![
                Instruction::ConstString { dest: 0, string },
                Instruction::ReturnVoid,
            ],
        )?)),
    });
    let bytes = common::write(&dex)?;
    let mut dex = common::parse_with_loading(&bytes, Loading::Deferred)?;
    let opened = ["dashboard opened"];
    assert_eq!(dex.find_methods_by_strings(&opened)?.len(), 1);
    let activity = dex.find_type_idx("Lexample/Activity;").expect("fixture");
    dex.remove_class(activity)?.expect("fixture");
    assert_eq!(dex.find_methods_by_strings(&opened)?.len(), 1);
    dex.intern_string("new pool entry");
    common::body(&mut dex).set_instructions(vec![Instruction::ReturnVoid]);
    assert!(dex.find_methods_by_strings(&opened)?.is_empty());
    Ok(())
}
