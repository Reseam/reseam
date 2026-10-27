// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

#[test]
fn edits_preserve_instruction_identity_through_nonlocal_expansion() {
    enum Edit {
        Insert,
        Grow,
    }
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
    let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    let guard =
        ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned())).unwrap();
    let class = fixture_class(0, "LRelocation;");
    for (input, edit) in [
        (fixtures::branch_marker(), Edit::Insert),
        (fixtures::invoke_frame(), Edit::Grow),
    ] {
        let method = methods::add_method(class, input);
        let mapping = match edit {
            Edit::Insert => mutation::insert_instructions(
                method,
                10,
                vec![Instruction::Simple(SimpleInsn { opcode: 0 }); 8],
            )
            .unwrap(),
            Edit::Grow => registers::grow_local_registers(method, 3, Vec::new()).unwrap(),
        };
        let instructions = search::get_instructions(method);
        let point = &instructions[mapping.instructions[1] as usize];
        match edit {
            Edit::Insert => {
                let Instruction::RegString(value) = point else {
                    panic!("surviving point no longer identifies its string instruction")
                };
                assert_eq!(value.value, "held");
            }
            Edit::Grow => {
                let Instruction::InvokeRange(value) = point else {
                    panic!("surviving point identifies staging instead of its invocation")
                };
                assert_eq!(value.method.name, "observe");
                assert!(mapping.starts[1] < mapping.instructions[1]);
                assert!(mapping.ends[1] > mapping.instructions[1]);
            }
        }
    }
    guard.finish().unwrap();
}

#[test]
fn flag_changes_keep_member_groups_frames_and_handles_consistent() {
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    let guard =
        ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned())).unwrap();
    let class = fixture_class(0, "LFlags;");
    let methods: Vec<_> = ["first", "second", "third"]
        .into_iter()
        .map(|name| methods::add_method(class, fixture_method(name.into(), "(I)V".into(), 1, 1)))
        .collect();
    for flags in [
        AccessFlags::PUBLIC,
        AccessFlags::PRIVATE,
        AccessFlags::PUBLIC,
    ] {
        methods::set_method_access_flags(methods[1], flags.bits()).unwrap();
        assert_eq!(
            lookup::class_methods_by_name(class, "second".into()),
            [methods[1]]
        );
        let info = lookup::get_method_info(methods[1]).unwrap();
        assert_eq!(info.ins_size, 2);
        assert!(info.register_count >= 2);
        let info = lookup::get_class_info(class).unwrap();
        assert_eq!(info.direct_method_count + info.virtual_method_count, 3);
    }
    methods::remove_method(methods[0]);
    for (name, handle) in ["second", "third"].into_iter().zip(&methods[1..]) {
        assert_eq!(lookup::class_methods_by_name(class, name.into()), [*handle]);
    }
    methods::set_method_access_flags(
        methods[1],
        (AccessFlags::PUBLIC | AccessFlags::ABSTRACT).bits(),
    )
    .unwrap();
    assert_eq!(
        lookup::get_method_info(methods[1]).unwrap().register_count,
        0
    );
    mutation::replace_body(
        methods[1],
        2,
        0,
        vec![Instruction::Simple(SimpleInsn { opcode: 0x0e })],
    )
    .unwrap();
    let info = lookup::get_method_info(methods[1]).unwrap();
    assert_eq!(info.ins_size, 2);
    assert!(!AccessFlags::from_bits_retain(info.access_flags).contains(AccessFlags::ABSTRACT));
    class_ops::remove_class(class);
    guard.finish().unwrap();
}

#[test]
fn field_storage_changes_preserve_unrelated_initial_values() {
    use crate::bridge::dex::types::{EncodedVal, NewField};
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    let guard =
        ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned())).unwrap();
    let class = fixture_class(0, "LFields;");
    for (name, value) in [("changed", 10), ("retained", 42)] {
        fields::add_field(
            class,
            NewField {
                name: name.into(),
                field_type: "I".into(),
                access_flags: (AccessFlags::PUBLIC | AccessFlags::STATIC).bits(),
                initial_value: Some(EncodedVal::IntVal(value)),
            },
        );
    }
    for flags in [
        AccessFlags::PUBLIC,
        AccessFlags::PUBLIC | AccessFlags::STATIC,
    ] {
        fields::set_field_access_flags(class, "changed".into(), flags.bits());
        let fields = lookup::class_fields(class);
        let retained = fields
            .iter()
            .find(|field| field.name == "retained")
            .unwrap();
        assert!(matches!(
            retained.initial_value,
            Some(EncodedVal::IntVal(42))
        ));
        let changed = fields.iter().find(|field| field.name == "changed").unwrap();
        assert_eq!(
            AccessFlags::from_bits_retain(changed.access_flags).contains(AccessFlags::STATIC),
            flags.contains(AccessFlags::STATIC)
        );
        assert!(changed.initial_value.is_none());
        let info = lookup::get_class_info(class).unwrap();
        assert_eq!(
            info.static_field_count,
            if flags.contains(AccessFlags::STATIC) {
                2
            } else {
                1
            }
        );
    }
    guard.finish().unwrap();
}
