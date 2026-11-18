// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::{self, TestResult};
use reseam_dex::{Loading, ParseOptions};

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
            let mut bytes = common::write(&dex)?;
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
        let bytes = common::write(&dex)?;
        reseam_dex::parse(&bytes, ParseOptions::default())?;
    }
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
        let encoded = common::write(&dex);
        if valid {
            reseam_dex::parse(&encoded?, ParseOptions::default())?;
        } else {
            assert!(encoded.is_err());
        }
    }
    Ok(())
}

#[test]
fn invocation_arguments_are_read_by_word() {
    use reseam_dex::{Instruction, MethodIdx, RegList};

    let list = Instruction::InvokeStatic {
        method: MethodIdx(0),
        args: RegList::from([4, 7]),
    };
    let range = Instruction::InvokeStaticRange {
        method: MethodIdx(0),
        first_reg: 20,
        count: 3,
    };

    assert_eq!(
        [0, 1, 2].map(|word| list.argument_register(word)),
        [Some(4), Some(7), None]
    );
    assert_eq!(
        [0, 2, 3].map(|word| range.argument_register(word)),
        [Some(20), Some(22), None]
    );
    assert_eq!(Instruction::ReturnVoid.argument_register(0), None);
}

#[test]
fn deferred_literal_queries_match_decoded_instructions() -> TestResult {
    use reseam_dex::{Instruction, RefKey, RefQuery};

    let mut dex = common::dex();
    let code = common::body(&mut dex);
    code.set_register_frame(10, 0, 0)?;
    let instructions = vec![
        Instruction::Const4 { dest: 0, value: -8 },
        Instruction::Const4 { dest: 1, value: 7 },
        Instruction::Const16 {
            dest: 2,
            value: i16::MIN,
        },
        Instruction::Const {
            dest: 3,
            value: i32::MIN,
        },
        Instruction::ConstHigh16 {
            dest: 4,
            value: -0x1234,
        },
        Instruction::ConstWide16 {
            dest: 4,
            value: -42,
        },
        Instruction::ConstWide32 {
            dest: 4,
            value: -100_000,
        },
        Instruction::ConstWide {
            dest: 4,
            value: -0x1234_5678_9abc_def0,
        },
        Instruction::ConstWideHigh16 {
            dest: 4,
            value: -0x7fff,
        },
        Instruction::AddIntLit16 {
            dest: 1,
            src: 0,
            literal: -3210,
        },
        Instruction::XorIntLit16 {
            dest: 1,
            src: 0,
            literal: 3210,
        },
        Instruction::AddIntLit8 {
            dest: 1,
            src: 0,
            literal: i8::MIN,
        },
        Instruction::UshrIntLit8 {
            dest: 1,
            src: 0,
            literal: i8::MAX,
        },
        Instruction::ReturnVoid,
    ];
    let expected: Vec<(usize, i64)> = instructions
        .iter()
        .enumerate()
        .filter_map(|(idx, insn)| Some((idx, insn.literal()?)))
        .collect();
    code.set_instructions(instructions);

    let bytes = common::write(&dex)?;
    let deferred = common::parse_with_loading(&bytes, Loading::Deferred)?;
    for &(expected_idx, literal) in &expected {
        let query = RefQuery::all_of([RefKey::Literal(literal)]);
        let hits = deferred.scan_instructions(&query, |site| {
            (site.instruction.literal() == Some(literal)).then_some(site.insn_idx)
        })?;
        assert_eq!(hits, vec![expected_idx], "literal {literal}");
    }
    Ok(())
}
