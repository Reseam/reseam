// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::reseam_dex::{AccessFlags, DexFile, DexHeader, DexVersion};

use super::{assembly, class_ops, lookup, mutation, registers, search};
use crate::context::PatchContext;
use crate::kotlin::handles::{ContextGuard, RunGuard};
use crate::kotlin::types::{Instruction, NewMethod, SimpleInsn};

fn fixture_method(name: String, proto: String, registers_size: u16, ins_size: u16) -> NewMethod {
    NewMethod {
        name,
        proto,
        registers_size,
        ins_size,
        access_flags: (AccessFlags::PUBLIC | AccessFlags::STATIC).bits(),
        outs_size: 0,
        instructions: vec![Instruction::Simple(SimpleInsn { opcode: 0x0e })],
        tries: Vec::new(),
        catch_handlers: Vec::new(),
    }
}

struct ExampleHandles {
    class: u32,
    method: u32,
    document: u32,
    root: u32,
}

fn example_handles(name: &str) -> ExampleHandles {
    let class = class_ops::create_class(
        0,
        format!("L{name};"),
        AccessFlags::PUBLIC.bits(),
        "Ljava/lang/Object;".into(),
    );
    let method =
        super::methods::add_method(class, fixture_method("run".into(), "()V".into(), 0, 0));
    let document = crate::kotlin::xml::xml_compile(format!("<{name}/>")).unwrap();
    let root = crate::kotlin::xml::xml_root(document);
    ExampleHandles {
        class,
        method,
        document,
        root,
    }
}

fn previous_run_handles() -> ExampleHandles {
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    let handles = example_handles("Old");
    guard.finish().unwrap();
    handles
}

#[test]
fn handles_from_finished_runs_cannot_alias_current_objects() {
    enum Thread {
        Current,
        Other,
    }
    for thread in [Thread::Current, Thread::Other] {
        let previous = match thread {
            Thread::Current => previous_run_handles(),
            Thread::Other => std::thread::spawn(previous_run_handles).join().unwrap(),
        };
        let (directory, mut apk) = crate::test_support::apk();
        apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
        let mut context = PatchContext::new(&mut apk);
        let _run = RunGuard::enter().unwrap();
        let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
        let current = example_handles("New");
        assert!(lookup::get_class_info(previous.class).is_none());
        assert!(lookup::get_method_info(previous.method).is_none());
        assert!(crate::kotlin::xml::xml_tag_name(previous.document, previous.root).is_empty());
        assert_eq!(
            lookup::get_class_info(current.class).unwrap().descriptor,
            "LNew;"
        );
        assert_eq!(
            lookup::get_method_info(current.method)
                .unwrap()
                .class_descriptor,
            "LNew;"
        );
        assert_eq!(
            crate::kotlin::xml::xml_tag_name(current.document, current.root),
            "New"
        );
        assert!(matches!(
            guard.finish(),
            Err(crate::error::PatcherError::Bridge(_))
        ));
    }
}

#[test]
fn handles_survive_class_removal_and_callback_boundaries() {
    let (dir, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
    let mut ctx = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut ctx, dir.path().to_owned()).unwrap();
    let classes: Vec<_> = ["LA;", "LB;", "LC;"]
        .into_iter()
        .map(|name| {
            let class = class_ops::create_class(
                0,
                name.into(),
                AccessFlags::PUBLIC.bits(),
                "Ljava/lang/Object;".into(),
            );
            let methods: Vec<_> = ["first", "second"]
                .into_iter()
                .map(|method| {
                    super::methods::add_method(
                        class,
                        NewMethod {
                            instructions: vec![Instruction::Simple(SimpleInsn { opcode: 0x0e })],
                            ..fixture_method(method.into(), "()V".into(), 0, 0)
                        },
                    )
                })
                .collect();
            (class, methods)
        })
        .collect();
    class_ops::remove_class(classes[1].0);
    for (descriptor, (class, methods)) in ["LA;", "LC;"].into_iter().zip([&classes[0], &classes[2]])
    {
        assert_eq!(lookup::find_class(descriptor.into()), Some(*class));
        for (name, method) in ["first", "second"].into_iter().zip(methods) {
            assert_eq!(
                lookup::find_method(descriptor.into(), name.into()),
                Some(*method)
            );
            assert_eq!(
                lookup::get_method_info(*method).unwrap().class_descriptor,
                descriptor
            );
        }
    }
    guard.finish().unwrap();
    let guard = ContextGuard::enter(&mut ctx, dir.path().to_owned()).unwrap();
    assert_eq!(
        lookup::get_class_info(classes[2].0).unwrap().descriptor,
        "LC;"
    );
    assert!(lookup::get_class_info(classes[1].0).is_none());
    assert!(lookup::get_method_info(classes[1].1[0]).is_none());
    assert!(guard.finish().is_err());
}
#[test]
fn pool_operands_survive_copying_between_dex_files() {
    use crate::kotlin::types::{EncodedVal, HandleRef};
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut other = DexFile::new(DexHeader::new(DexVersion::V039));
    other
        .intern_method("LDifferent;", "unrelated", "(Z)I")
        .unwrap();
    apk.add_dex(other);
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    let inputs = crate::test_support::pool_operands();
    let methods: Vec<_> = [(0, "LSource;"), (1, "LDestination;")]
        .into_iter()
        .map(|(dex, name)| {
            let class = class_ops::create_class(
                dex,
                name.into(),
                AccessFlags::PUBLIC.bits(),
                "Ljava/lang/Object;".into(),
            );
            super::methods::add_method(
                class,
                NewMethod {
                    outs_size: 4,
                    instructions: if dex == 0 {
                        inputs.clone()
                    } else {
                        vec![Instruction::Simple(SimpleInsn { opcode: 0x0e })]
                    },
                    ..fixture_method("run".into(), "()V".into(), 4, 0)
                },
            )
        })
        .collect();
    guard.finish().unwrap();
    let source = crate::kotlin::handles::method_location(methods[0]).unwrap();
    crate::test_support::duplicate_call_site(&mut context, source);
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    mutation::set_instructions(methods[1], search::get_instructions(methods[0])).unwrap();
    for &method in &methods {
        let instructions = search::get_instructions(method);
        let Instruction::RegProto(proto) = &instructions[0] else {
            panic!("method type operand was lost")
        };
        assert_eq!(proto.proto, "(IJ)Ljava/lang/String;");
        let Instruction::RegHandle(handle) = &instructions[1] else {
            panic!("method handle operand was lost")
        };
        let HandleRef::Method(handle) = &handle.handle else {
            panic!("handle member changed")
        };
        assert_eq!(handle.method.defining_class, "LBootstrap;");
        let Instruction::Polymorphic(call) = &instructions[2] else {
            panic!("polymorphic prototype was lost")
        };
        assert_eq!(call.proto, "(IJ)Ljava/lang/String;");
        assert_eq!(call.method.proto, "([Ljava/lang/Object;)Ljava/lang/Object;");
        let Instruction::Custom(call) = &instructions[3] else {
            panic!("call site was lost")
        };
        assert_eq!(call.call_site.name, "dynamic");
        let EncodedVal::ArrayVal(arguments) = &call.call_site.arguments[1] else {
            panic!("bootstrap array was lost")
        };
        let EncodedVal::FloatVal(nan) = arguments[1] else {
            panic!("bootstrap scalar kind changed")
        };
        assert_eq!(nan.to_bits(), 0x7fc0_0001);
        let EncodedVal::DoubleVal(zero) = arguments[2] else {
            panic!("bootstrap scalar kind changed")
        };
        assert_eq!(zero.to_bits(), (-0.0_f64).to_bits());
    }
    mutation::set_instructions(methods[0], search::get_instructions(methods[0])).unwrap();
    guard.finish().unwrap();
    for dex in [0, 1] {
        assert_eq!(context.dex_file(dex).unwrap().call_sites().len(), 2);
    }
}
#[test]
fn authored_raw_pool_references_survive_pure_lowering_and_insertion() {
    enum Reference {
        String,
        Type,
    }
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    let handles = example_handles("RawReferences");
    registers::grow_local_registers(handles.method, 1, vec![]).unwrap();
    let references = [
        (
            Reference::String,
            (0x1a, super::pools::intern_string(0, "held string".into())),
        ),
        (
            Reference::Type,
            (0x1c, super::pools::intern_type(0, "LHeldType;".into())),
        ),
    ];
    guard.finish().unwrap();
    for (reference, (opcode, index)) in references {
        let index = u16::try_from(index).unwrap().to_le_bytes();
        let raw = Instruction::Raw(vec![opcode, 0, index[0], index[1]]);
        let lowered = assembly::lower_instructions(vec![raw], vec![]).unwrap();
        let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
        mutation::insert_instructions(handles.method, 0, lowered).unwrap();
        match reference {
            Reference::String => assert_eq!(
                search::instruction_string_ref(handles.method, 0).as_deref(),
                Some("held string")
            ),
            Reference::Type => assert_eq!(
                search::instruction_type_ref(handles.method, 0).as_deref(),
                Some("LHeldType;")
            ),
        }
        guard.finish().unwrap();
    }
}

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
        let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
        let class = class_ops::create_class(
            0,
            "LPayloads;".into(),
            AccessFlags::PUBLIC.bits(),
            "Ljava/lang/Object;".into(),
        );
        let method = super::methods::add_method(
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
                reseam_apk::reseam_dex::Instruction::FillArrayData { payload_offset, .. } => {
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
        let reseam_apk::reseam_dex::Instruction::FillArrayDataPayload(payload) = target else {
            panic!("array initializer targets something other than its data")
        };
        assert_eq!(&payload.data[..], &[7]);
    }
}

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
        (false, |_handles| {
            super::pools::intern_string(1, "text".into());
        }),
        (false, |_handles| {
            super::pools::get_string(0, u32::MAX);
        }),
        (false, |_handles| {
            super::pools::get_type_descriptor(0, u32::MAX);
        }),
        (false, |_handles| {
            super::pools::intern_type(0, "not a descriptor".into());
        }),
        (false, |handles| {
            super::fields::set_static_field_value(
                handles.class,
                "missing".into(),
                crate::kotlin::types::EncodedVal::IntVal(1),
            );
        }),
        (false, |handles| {
            super::annotations::add_field_annotation(handles.class, "missing".into(), annotation());
        }),
        (true, |handles| {
            search::instruction_type_ref(handles.method, 0);
        }),
        (false, |handles| {
            search::instruction_type_ref(handles.method, 1);
        }),
        (false, |_handles| {
            search::find_method_call_sites(vec!["LRequired;".into()], vec![]);
        }),
        (false, |_handles| {
            search::find_field_access_sites(vec![], vec!["value".into()]);
        }),
        (false, |_handles| {
            lookup::find_methods_by_opcodes(vec![i32::MAX]);
        }),
        (false, |handles| {
            search::index_of_first_field_access(handles.method, i32::MAX, None, None, 0);
        }),
        (false, |handles| {
            super::annotations::add_class_annotation(
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
        let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
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
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    let class = class_ops::create_class(
        0,
        "LSubject;".into(),
        AccessFlags::PUBLIC.bits(),
        "Ljava/lang/Object;".into(),
    );
    let method =
        super::methods::add_method(class, fixture_method("run".into(), "()V".into(), 32, 0));
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
        let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
        assert!(mutation::set_instructions(method, vec![instruction]).is_err());
        assert!(guard.finish().is_err());
    }
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    search::get_instruction(method, 1000);
    assert!(guard.finish().is_err());
}
#[test]
fn edits_preserve_instruction_identity_through_nonlocal_expansion() {
    enum Edit {
        Insert,
        Grow,
    }
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    let class = class_ops::create_class(
        0,
        "LRelocation;".into(),
        AccessFlags::PUBLIC.bits(),
        "Ljava/lang/Object;".into(),
    );
    for (input, edit) in [
        (crate::test_support::branch_marker(), Edit::Insert),
        (crate::test_support::invoke_frame(), Edit::Grow),
    ] {
        let method = super::methods::add_method(class, input);
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
fn assembling_blocks_keeps_branches_over_lowered_invocations() {
    use crate::kotlin::types::{Branch0Insn, InvokeInsn, MethodRef, ScratchSpan};
    use reseam_apk::reseam_dex::Instruction as NativeInstruction;

    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    let class = class_ops::create_class(
        0,
        "LAssembly;".into(),
        AccessFlags::PUBLIC.bits(),
        "Ljava/lang/Object;".into(),
    );
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
        let method = super::methods::add_method(
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
fn flag_changes_keep_member_groups_frames_and_handles_consistent() {
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    let class = class_ops::create_class(
        0,
        "LFlags;".into(),
        AccessFlags::PUBLIC.bits(),
        "Ljava/lang/Object;".into(),
    );
    let methods: Vec<_> = ["first", "second", "third"]
        .into_iter()
        .map(|name| {
            super::methods::add_method(class, fixture_method(name.into(), "(I)V".into(), 1, 1))
        })
        .collect();
    for flags in [
        AccessFlags::PUBLIC,
        AccessFlags::PRIVATE,
        AccessFlags::PUBLIC,
    ] {
        super::methods::set_method_access_flags(methods[1], flags.bits()).unwrap();
        assert_eq!(
            lookup::find_method("LFlags;".into(), "second".into()),
            Some(methods[1])
        );
        assert_eq!(registers::ins_size(methods[1]), 2);
        assert!(registers::registers_size(methods[1]) >= 2);
        let info = lookup::get_class_info(class).unwrap();
        assert_eq!(info.direct_method_count + info.virtual_method_count, 3);
    }
    super::methods::remove_method(methods[0]);
    for (name, handle) in ["second", "third"].into_iter().zip(&methods[1..]) {
        assert_eq!(
            lookup::find_method("LFlags;".into(), name.into()),
            Some(*handle)
        );
    }
    super::methods::set_method_access_flags(
        methods[1],
        (AccessFlags::PUBLIC | AccessFlags::ABSTRACT).bits(),
    )
    .unwrap();
    assert_eq!(registers::registers_size(methods[1]), 0);
    mutation::replace_body(
        methods[1],
        2,
        0,
        vec![Instruction::Simple(SimpleInsn { opcode: 0x0e })],
    )
    .unwrap();
    assert_eq!(registers::ins_size(methods[1]), 2);
    assert!(
        !AccessFlags::from_bits_retain(lookup::get_method_info(methods[1]).unwrap().access_flags)
            .contains(AccessFlags::ABSTRACT)
    );
    class_ops::remove_class(class);
    guard.finish().unwrap();
}

#[test]
fn field_storage_changes_preserve_unrelated_initial_values() {
    use crate::kotlin::types::{EncodedVal, NewField};
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut context, directory.path().to_owned()).unwrap();
    let class = class_ops::create_class(
        0,
        "LFields;".into(),
        AccessFlags::PUBLIC.bits(),
        "Ljava/lang/Object;".into(),
    );
    for (name, value) in [("changed", 10), ("retained", 42)] {
        super::fields::add_field(
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
        super::fields::set_field_access_flags(class, "changed".into(), flags.bits());
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
