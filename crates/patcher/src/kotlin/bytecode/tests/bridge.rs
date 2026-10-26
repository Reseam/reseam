// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

fn annotation() -> crate::kotlin::types::AnnotationItem {
    crate::kotlin::types::AnnotationItem {
        visibility: 0,
        annotation_type: "LAnnotation;".into(),
        elements: Vec::new(),
    }
}

#[test]
fn bridge_queries_distinguish_absence_from_invalid_requests() {
    type Request = fn(&ExampleHandles);
    let cases: &[(bool, Request)] = &[
        (false, |handles| {
            registers::instruction_register(handles.method, 1, 0);
        }),
        (false, |handles| {
            registers::instruction_register(handles.method, 0, 0);
        }),
        (false, |handles| {
            registers::instruction_wide_literal(handles.method, 0);
        }),
        (false, |handles| {
            fields::set_static_field_value(
                handles.class,
                "missing".into(),
                crate::kotlin::types::EncodedVal::IntVal(1),
            );
        }),
        (false, |handles| {
            annotations::add_field_annotation(handles.class, "missing".into(), annotation());
        }),
        (true, |handles| {
            search::instruction_type_ref(handles.method, 0);
        }),
        (false, |handles| {
            search::instruction_type_ref(handles.method, 1);
        }),
        (false, |_handles| {
            lookup::find_methods_by_opcodes(vec![i32::MAX]);
        }),
        (false, |handles| {
            positions::index_of_first_field_access(handles.method, i32::MAX, None, None, 0);
        }),
        (false, |handles| {
            annotations::add_class_annotation(
                handles.class,
                crate::kotlin::types::AnnotationItem {
                    visibility: 3,
                    ..annotation()
                },
            );
        }),
    ];
    for &(optional_absence, request) in cases {
        let (directory, mut apk) = crate::test_support::apk();
        apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
        let mut context = PatchContext::new(&mut apk);
        let _run = RunGuard::enter().unwrap();
        let guard =
            ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned()))
                .unwrap();
        request(&example_handles("Required"));
        assert_eq!(guard.finish().is_ok(), optional_absence);
    }
}

#[test]
fn invalid_instruction_operands_fail_the_callback() {
    use crate::kotlin::types::{Reg1Insn, Reg2Insn, RegLiteralInsn, SparseSwitchInsn};
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard =
        ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned())).unwrap();
    let class = fixture_class(0, "LSubject;");
    let method = methods::add_method(class, fixture_method("run".into(), "()V".into(), 32, 0));
    guard.finish().unwrap();
    for instruction in [
        Instruction::Simple(SimpleInsn { opcode: 0x12 }),
        Instruction::Reg1(Reg1Insn {
            opcode: 0x12,
            reg_a: 0,
        }),
        Instruction::RegLiteral(RegLiteralInsn {
            opcode: 0x12,
            reg_a: 0,
            reg_b: 0,
            literal: 32,
        }),
        Instruction::Reg2(Reg2Insn {
            opcode: 0x01,
            reg_a: 16,
            reg_b: 1,
        }),
        Instruction::SparseSwitchData(SparseSwitchInsn {
            keys: vec![1],
            targets: Vec::new(),
        }),
        Instruction::Raw(vec![0]),
        Instruction::Raw(vec![0, 0, 0x0e, 0]),
    ] {
        let guard =
            ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned()))
                .unwrap();
        assert!(mutation::set_instructions(method, vec![instruction]).is_err());
        assert!(guard.finish().is_err());
    }
}
