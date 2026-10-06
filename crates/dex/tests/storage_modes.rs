// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod common;

use reseam_dex::{Loading, ParseOptions};
use std::io::Read;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn streamed_resident_and_spooled_writers_preserve_the_same_program() -> TestResult {
    let input = reseam_dex::write(&common::dex())?;
    let mut streamed = reseam_dex::parse(
        &input,
        ParseOptions {
            classes: Loading::Deferred,
            debug_info: Loading::Deferred,
            annotations: Loading::Deferred,
            ..ParseOptions::default()
        },
    )?;
    let mut resident = reseam_dex::parse(&input, ParseOptions::default())?;
    streamed.intern_string("!first sorted string");
    resident.intern_string("!first sorted string");
    let expected = reseam_dex::write(&resident)?;
    assert_eq!(reseam_dex::write(&streamed)?, expected);
    let spool = reseam_dex::write_spooled(&streamed, None)?;
    let mut actual = Vec::new();
    spool.reader().read_to_end(&mut actual)?;
    assert_eq!(actual, expected);
    reseam_dex::parse(&actual, ParseOptions::default())?;
    Ok(())
}

#[test]
fn truncated_files_fail_validation() -> TestResult {
    let bytes = reseam_dex::write(&common::dex())?;
    for length in [0, 8, 111, bytes.len() - 1] {
        assert!(reseam_dex::parse(&bytes[..length], ParseOptions::default()).is_err());
    }
    Ok(())
}

#[test]
fn edits_relocate_debug_and_handlers_and_roll_back_invalid_targets() -> TestResult {
    use reseam_dex::types::debug::DebugBytecode;
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
        code.replace_instruction(0, Instruction::Goto { offset: 1 })
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
    code.insert_instruction(
        1,
        Instruction::Const {
            dest: 1,
            value: 700,
        },
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
    code.set_debug_info(Some(reseam_dex::types::metadata::Metadata::new(
        reseam_dex::DebugInfo {
            line_start: 42,
            parameter_names: Vec::new(),
            bytecodes: vec![
                DebugBytecode::AdvancePc { advance: 1 },
                DebugBytecode::EndSequence,
            ],
        },
    )));
    code.insert_instruction(0, Instruction::Nop)?;
    let bytes = reseam_dex::write(&dex)?;
    reseam_dex::parse(&bytes, ParseOptions::default())?;
    Ok(())
}

#[test]
fn prototype_identities_survive_global_pool_overflow_and_part_compaction() -> TestResult {
    use reseam_dex::{AccessFlags, EncodedMethod};
    let mut dex = common::dex();
    let mut descriptor = String::new();
    for value in 0..=u16::MAX {
        descriptor.clear();
        descriptor.push('(');
        descriptor.extend((0..16).map(|bit| if value & (1 << bit) == 0 { 'B' } else { 'I' }));
        descriptor.push_str(")V");
        dex.intern_proto(&descriptor)?;
    }
    let prototype = dex.intern_proto(&descriptor)?;
    assert!(prototype.0 > u32::from(u16::MAX));
    let method = dex.intern_method("Lexample/Activity;", "nativeCall", &descriptor)?;
    dex.class_mut(0)?.add_direct_method(EncodedMethod {
        method,
        access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC | AccessFlags::NATIVE,
        code: None,
    });
    assert!(reseam_dex::write(&dex).is_err());
    let parts = reseam_dex::split_to_fit(&dex)?.expect("fixture");
    assert_eq!(parts.len(), 1);
    let spool = reseam_dex::write_spooled(&dex, Some(&parts[0]))?;
    let mut bytes = Vec::new();
    spool.reader().read_to_end(&mut bytes)?;
    let parsed = reseam_dex::parse(&bytes, ParseOptions::default())?;
    let method = parsed
        .find_method_by_name("nativeCall")?
        .expect("fixture")
        .method;
    assert_eq!(
        parsed.proto_descriptor(&parsed.proto(parsed.method_id(method).proto)),
        descriptor
    );
    Ok(())
}

#[test]
fn descriptor_policies_match_resident_and_deferred_methods_consistently() -> TestResult {
    use reseam_dex::{Fingerprint, TypePattern};
    let bytes = reseam_dex::write(&common::dex())?;
    for loading in [Loading::Eager, Loading::Deferred] {
        let dex = common::parse_with_loading(&bytes, loading)?;
        for (pattern, count) in [
            (TypePattern::Exact("V".into()), 1),
            (TypePattern::Exact("Ljava/lang/Object;".into()), 0),
            (TypePattern::Prefix("Ljava/".into()), 1),
            (TypePattern::Object, 1),
            (TypePattern::Array, 0),
        ] {
            assert_eq!(
                dex.find_methods_by_fingerprint(&Fingerprint {
                    return_type: Some(pattern),
                    ..Fingerprint::default()
                })?
                .len(),
                count
            );
        }
    }
    Ok(())
}

#[test]
fn table_counts_and_offsets_are_checked_against_the_declared_source_span() -> TestResult {
    let bytes = reseam_dex::write(&common::dex())?;
    let options = common::unchecked();
    for count_at in [0x38, 0x40, 0x48, 0x50, 0x58, 0x60] {
        let mut invalid = bytes.clone();
        invalid[count_at..count_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(reseam_dex::parse(&invalid, options).is_err());
        let mut invalid = bytes.clone();
        let outside = invalid.len() as u32;
        invalid.extend_from_slice(&bytes);
        invalid[count_at + 4..count_at + 8].copy_from_slice(&outside.to_le_bytes());
        assert!(reseam_dex::parse(&invalid, options).is_err());
    }
    Ok(())
}

#[test]
fn invocation_arguments_require_an_encoding_that_can_represent_them() -> TestResult {
    use reseam_dex::Instruction;
    for count in [0, 1, 5, 6] {
        let mut dex = common::dex();
        let descriptor = format!("({})V", "I".repeat(count));
        let method = dex.intern_method("Lexample/Activity;", "callee", &descriptor)?;
        let code = common::body(&mut dex);
        code.set_register_frame(count.max(2) as u16, 0, 0)?;
        let arguments = reseam_dex::RegList::try_from_iter(0..count as u8);
        assert_eq!(arguments.is_ok(), count <= 5);
        if let Ok(args) = arguments {
            code.set_instructions(vec![
                Instruction::InvokeStatic { method, args },
                Instruction::ReturnVoid,
            ]);
            let mut bytes = reseam_dex::write(&dex)?;
            let deferred = common::parse_with_loading(&bytes, Loading::Deferred)?;
            let code_offset =
                deferred.class_skeleton(0)?.expect("fixture").direct_methods[0].code_off as usize;
            bytes[code_offset + 17] = (bytes[code_offset + 17] & 0x0f) | 0x60;
            assert!(reseam_dex::parse(&bytes, common::unchecked()).is_err());
        }
        let code = common::body(&mut dex);
        code.set_instructions(vec![
            Instruction::InvokeStaticRange {
                method,
                first_reg: 0,
                count: count as u8,
            },
            Instruction::ReturnVoid,
        ]);
        let bytes = reseam_dex::write(&dex)?;
        reseam_dex::parse(&bytes, ParseOptions::default())?;
    }
    Ok(())
}

#[test]
fn invalid_descriptors_leave_type_and_string_pools_unchanged() {
    let mut dex = common::dex();
    let types = dex.type_count();
    let strings = dex.strings().len();
    for descriptor in ["", "L;", "[V", "Lmissing", "II"] {
        assert!(dex.intern_type(descriptor).is_err());
        assert_eq!(dex.type_count(), types);
        assert_eq!(dex.strings().len(), strings);
    }
}

#[test]
fn call_sites_and_method_handles_survive_deferred_reads_appends_and_remapping() -> TestResult {
    use reseam_dex::types::method_handle::{MethodHandleMember, MethodHandleType};
    use reseam_dex::{CallSiteItem, EncodedValue, Instruction, MethodHandle};
    let mut dex = common::dex();
    let bootstrap = dex.intern_method("Lexample/Bootstrap;", "bootstrap", "()V")?;
    let handle = dex.method_handles_mut().push(MethodHandle {
        handle_type: MethodHandleType::InvokeStatic,
        member: MethodHandleMember::Method(bootstrap),
    });
    let site = CallSiteItem {
        bootstrap_method: reseam_dex::MethodHandleIdx(handle as u32),
        method_name: dex.intern_string("dynamicCall"),
        method_type: dex.intern_proto("()V")?,
        extra_arguments: vec![EncodedValue::String(dex.intern_string("argument"))],
    };
    let index = dex.call_sites_mut().push(site);
    common::body(&mut dex).set_instructions(vec![
        Instruction::InvokeCustom {
            call_site: reseam_dex::CallSiteIdx(index as u32),
            args: reseam_dex::RegList::default(),
        },
        Instruction::ReturnVoid,
    ]);
    let input = reseam_dex::write(&dex)?;
    let mut parsed = common::parse_with_loading(&input, Loading::Deferred)?;
    assert!(parsed.call_sites().get(1).is_err());
    let mut appended = parsed.call_sites().get(0)?.into_owned();
    appended.method_name = parsed.intern_string("!appendedCall");
    parsed.call_sites_mut().push(appended);
    let bytes = reseam_dex::write(&parsed)?;
    let result = reseam_dex::parse(&bytes, ParseOptions::default())?;
    assert_eq!(result.call_sites().len(), 2);
    for (index, name) in [(0, "dynamicCall"), (1, "!appendedCall")] {
        let site = result.call_sites().get(index)?;
        assert_eq!(result.string(site.method_name), name);
        let EncodedValue::String(argument) = site.extra_arguments[0] else {
            panic!("string argument");
        };
        assert_eq!(result.string(argument), "argument");
        let handle = result
            .method_handles()
            .get(site.bootstrap_method.0 as usize)?;
        let MethodHandleMember::Method(target) = handle.member else {
            panic!("bootstrap method");
        };
        assert_eq!(result.string(result.method_id(target).name), "bootstrap");
    }
    Ok(())
}

#[test]
fn invalid_pool_references_fail_before_lookup_or_serialization() -> TestResult {
    use reseam_dex::{Instruction, MethodIdx, StringIdx};
    let bytes = reseam_dex::write(&common::dex())?;
    let parsed = reseam_dex::parse(&bytes, ParseOptions::default())?;
    let header = parsed.header();
    let options = common::unchecked();
    for offset in [
        header.type_ids_off,
        header.proto_ids_off + 4,
        header.field_ids_off + 4,
        header.method_ids_off + 4,
        header.class_defs_off,
    ] {
        let mut invalid = bytes.clone();
        invalid[offset as usize..offset as usize + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(reseam_dex::parse(&invalid, options).is_err());
    }
    let mut dex = common::dex();
    let method = common::direct_method(&mut dex);
    method.method = MethodIdx(u32::MAX);
    assert!(dex.find_method_by_name("report").is_err());
    assert!(reseam_dex::write(&dex).is_err());
    let mut dex = common::dex();
    common::body(&mut dex).set_instructions(vec![
        Instruction::ConstString {
            dest: 0,
            string: StringIdx(u32::MAX),
        },
        Instruction::ReturnVoid,
    ]);
    assert!(reseam_dex::write_spooled(&dex, None).is_err());
    Ok(())
}

#[test]
fn array_payloads_require_complete_elements_with_nonzero_width() -> TestResult {
    use reseam_dex::{FillArrayPayloadData, Instruction};
    for (width, data) in [
        (0, vec![]),
        (2, vec![1, 2, 3]),
        (1, vec![1, 2, 3]),
        (2, vec![1, 2, 3, 4]),
    ] {
        let valid = width != 0 && data.len().is_multiple_of(width as usize);
        let mut dex = common::dex();
        let type_ = dex.intern_type(if width == 2 { "[S" } else { "[B" })?;
        common::body(&mut dex).set_instructions(vec![
            Instruction::Const4 { dest: 0, value: 3 },
            Instruction::NewArray {
                dest: 1,
                size: 0,
                type_,
            },
            Instruction::FillArrayData {
                array: 1,
                payload_offset: 5,
            },
            Instruction::ReturnVoid,
            Instruction::Nop,
            Instruction::FillArrayDataPayload(Box::new(FillArrayPayloadData {
                element_width: width,
                data,
            })),
        ]);
        let encoded = reseam_dex::write(&dex);
        if valid {
            reseam_dex::parse(&encoded?, ParseOptions::default())?;
        } else {
            assert!(encoded.is_err());
        }
    }
    Ok(())
}

#[test]
fn fingerprint_opcode_windows_agree_for_deferred_and_resident_methods() -> TestResult {
    use reseam_dex::{Fingerprint, InstructionPattern, OpcodeMatcher};
    let bytes = reseam_dex::write(&common::dex())?;
    for loading in [Loading::Deferred, Loading::Eager] {
        let dex = common::parse_with_loading(&bytes, loading)?;
        let fingerprint = Fingerprint {
            name: Some("report".into()),
            opcodes: Some(vec![
                InstructionPattern::Opcode(OpcodeMatcher::ConstString),
                InstructionPattern::Opcode(OpcodeMatcher::ReturnVoid),
            ]),
            ..Fingerprint::default()
        };
        let hit = dex
            .find_method_by_fingerprint(&fingerprint)?
            .expect("fixture");
        assert_eq!(hit.matched_indices, vec![0, 1]);
        let mismatch = Fingerprint {
            opcodes: Some(vec![InstructionPattern::Opcode(
                OpcodeMatcher::ReturnObject,
            )]),
            ..fingerprint
        };
        assert!(dex.find_method_by_fingerprint(&mismatch)?.is_none());
    }
    Ok(())
}

#[test]
fn cached_reference_search_tracks_resident_edits_and_class_removal() -> TestResult {
    use reseam_dex::{AccessFlags, CodeItem, EncodedMethod, Fingerprint, Instruction};
    let mut dex = common::dex();
    let class = dex.create_class(
        "Lexample/Dashboard;",
        AccessFlags::PUBLIC,
        Some("Ljava/lang/Object;"),
    )?;
    let string = dex.intern_string("dashboard opened");
    let method = dex.intern_method("Lexample/Dashboard;", "report", "()V")?;
    dex.class_mut(class)?.add_direct_method(EncodedMethod {
        method,
        access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
        code: Some(CodeItem::new(
            1,
            0,
            0,
            vec![
                Instruction::ConstString { dest: 0, string },
                Instruction::ReturnVoid,
            ],
        )?),
    });
    let bytes = reseam_dex::write(&dex)?;
    let mut dex = common::parse_with_loading(&bytes, Loading::Deferred)?;
    let fingerprint = Fingerprint {
        strings: Some(vec!["dashboard opened".into()]),
        ..Fingerprint::default()
    };
    assert_eq!(dex.find_methods_by_fingerprint(&fingerprint)?.len(), 1);
    let activity = dex.find_type_idx("Lexample/Activity;").expect("fixture");
    dex.remove_class(activity)?.expect("fixture");
    assert_eq!(dex.find_methods_by_fingerprint(&fingerprint)?.len(), 1);
    dex.intern_string("new pool entry");
    common::body(&mut dex).set_instructions(vec![Instruction::ReturnVoid]);
    assert!(dex.find_methods_by_fingerprint(&fingerprint)?.is_empty());
    Ok(())
}

#[test]
fn android_string_encodings_survive_lookup_interning_and_serialization() -> TestResult {
    let text = ["", "a", "a\0b", "日本語", "𝄞", "\u{ffff}"];
    let mut dex = common::dex();
    for value in text {
        dex.intern_string(value);
    }
    let bytes = reseam_dex::write(&dex)?;
    for loading in [Loading::Eager, Loading::Deferred] {
        let mut dex = common::parse_with_loading(&bytes, loading)?;
        assert_eq!(reseam_dex::write(&dex)?, bytes);
        dex.intern_string("!new");
        let output = reseam_dex::write(&dex)?;
        let result = reseam_dex::parse(&output, ParseOptions::default())?;
        for value in text {
            let index = result.find_string_idx(value).expect("fixture");
            assert_eq!(result.string(index), value);
        }
    }
    let mut dex =
        reseam_dex::DexFile::new(reseam_dex::DexHeader::new(reseam_dex::DexVersion::V039));
    dex.intern_string("\u{fffd}");
    let mut bytes = reseam_dex::write(&dex)?;
    let parsed = reseam_dex::parse(&bytes, ParseOptions::default())?;
    let ids = parsed.header().string_ids_off as usize;
    let data = u32::from_le_bytes(bytes[ids..ids + 4].try_into()?) as usize;
    let options = common::unchecked();
    for length in [0, 2, 127] {
        let mut invalid = bytes.clone();
        invalid[data] = length;
        assert!(reseam_dex::parse(&invalid, options).is_err());
    }
    bytes[data + 1..data + 4].copy_from_slice(&[0xed, 0xa0, 0x80]);
    let mut dex = reseam_dex::parse(&bytes, options)?;
    assert!(dex.find_string_idx("\u{fffd}").is_none());
    dex.intern_string("\u{fffd}");
    let output = reseam_dex::write(&dex)?;
    let result = reseam_dex::parse(&output, ParseOptions::default())?;
    assert_eq!(result.strings().len(), 2);
    assert!(
        output
            .windows(5)
            .any(|item| item == [1, 0xed, 0xa0, 0x80, 0])
    );
    Ok(())
}
