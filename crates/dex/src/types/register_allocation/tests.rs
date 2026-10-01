// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;
use crate::{CatchHandler, DexHeader, DexVersion, TryItem};

fn dex() -> DexFile {
    DexFile::new(DexHeader::new(DexVersion::V035))
}

fn code(ins_size: u16, instructions: Vec<Instruction>) -> CodeItem {
    CodeItem {
        registers_size: 16,
        ins_size,
        outs_size: 0,
        debug_info: None,
        instructions,
        tries: vec![],
        catch_handlers: vec![],
    }
}

#[test]
fn lowers_wide_field_access_and_relocates_branches_and_handlers() {
    use Instruction::*;
    let mut dex = dex();
    let field = dex.intern_field("LExample;", "value", "J").unwrap();
    let mut code = code(
        1,
        vec![
            ConstWide16 { dest: 12, value: 7 },
            IfEqz { a: 15, offset: 3 },
            Nop,
            IgetWide {
                dest: 12,
                obj: 15,
                field,
            },
            ReturnWide { src: 12 },
            MoveException { dest: 0 },
            ReturnWide { src: 12 },
        ],
    );
    code.tries.push(TryItem {
        start_addr: 5,
        insn_count: 2,
        handler_idx: 0,
    });
    code.catch_handlers.push(CatchHandler {
        typed_catches: vec![],
        catch_all_addr: Some(8),
    });
    let indices = grow_registers(
        &mut code,
        3,
        &[dex
            .intern_type("LExample;")
            .expect("valid fixture descriptor")],
        &[],
        &dex,
    )
    .unwrap();
    assert_eq!(code.registers_size, 19);
    let MoveObjectFrom16 { dest, src } = code.instructions[indices.starts()[3]] else {
        panic!("object operand requires staging");
    };
    assert_eq!(src, 18);
    assert_eq!(
        code.instructions[indices.starts()[3] + 1],
        IgetWide {
            dest: 12,
            obj: dest,
            field
        }
    );
    assert_eq!(
        code.instructions[indices.starts()[1]],
        IfEqz { a: 18, offset: 3 }
    );
    assert_eq!(code.tries[0].start_addr, 5);
    assert_eq!(code.tries[0].insn_count, 4);
    assert_eq!(code.catch_handlers[0].catch_all_addr, Some(10));
}

#[test]
fn a_protected_register_is_never_staged_through() {
    use Instruction::*;
    let mut dex = dex();
    let field = dex.intern_field("LExample;", "value", "J").unwrap();
    let body = vec![
        Const4 { dest: 0, value: 0 },
        IgetWide {
            dest: 12,
            obj: 15,
            field,
        },
        ReturnWide { src: 12 },
    ];
    for protected in [&[][..], &[0][..]] {
        let mut code = code(1, body.clone());
        grow_registers(
            &mut code,
            3,
            &[dex.intern_type("LExample;").expect("type")],
            protected,
            &dex,
        )
        .unwrap();
        let MoveObjectFrom16 { dest, src } = code.instructions[1] else {
            panic!("object staging");
        };
        assert_eq!(src, 18);
        assert!(!protected.contains(&u16::from(dest)));
        assert!(
            matches!(code.instructions[2], IgetWide { obj, field: actual, .. } if obj == dest && actual == field)
        );
    }
}

#[test]
fn register_growth_stages_object_operands_beyond_encoding_limits() {
    use Instruction::*;
    struct Case {
        registers: u16,
        incoming: u16,
        additional: u16,
        instructions: Vec<Instruction>,
    }
    let mut dex = dex();
    let object = dex.intern_type("LExample;").expect("object type");
    let field = dex
        .intern_field("LExample;", "items", "Ljava/util/List;")
        .unwrap();
    for case in [
        Case {
            registers: 16,
            incoming: 2,
            additional: 3,
            instructions: vec![
                IfEq {
                    a: 14,
                    b: 15,
                    offset: 3,
                },
                ReturnVoid,
                ReturnVoid,
            ],
        },
        Case {
            registers: 2,
            incoming: 1,
            additional: 32,
            instructions: vec![
                IgetObject {
                    dest: 0,
                    obj: 1,
                    field,
                },
                ReturnObject { src: 0 },
            ],
        },
    ] {
        let mut code = code(case.incoming, case.instructions);
        code.registers_size = case.registers;
        let incoming = vec![object; usize::from(case.incoming)];
        grow_registers(&mut code, case.additional, &incoming, &[], &dex).unwrap();
        assert_eq!(code.registers_size(), case.registers + case.additional);
        let moves: Vec<_> = code
            .instructions()
            .iter()
            .filter_map(|instruction| match instruction {
                MoveObjectFrom16 { dest, src } => Some((u16::from(*dest), *src)),
                _ => None,
            })
            .collect();
        assert_eq!(moves.len(), usize::from(case.incoming));
        for (word, &(dest, src)) in moves.iter().enumerate() {
            assert!(dest <= 15);
            assert_eq!(
                src,
                case.registers + case.additional - case.incoming + word as u16
            );
        }
        match &code.instructions()[moves.len()] {
            IfEq { a, b, offset } => {
                assert_eq!((u16::from(*a), u16::from(*b)), (moves[0].0, moves[1].0));
                assert_eq!(*offset, 3);
            }
            IgetObject {
                obj, field: actual, ..
            } => {
                assert_eq!(u16::from(*obj), moves[0].0);
                assert_eq!(*actual, field);
            }
            _ => panic!("original operation survives growth"),
        }
    }
}

#[test]
fn failure_preserves_code_instead_of_clobbering_a_live_register() {
    use Instruction::*;
    let mut dex = dex();
    let field = dex.intern_field("LExample;", "value", "I").unwrap();
    let method = dex
        .intern_method("LExample;", "consume", "(IIIIIIIIIIIIIIILExample;)V")
        .unwrap();
    let mut code = code(
        16,
        vec![
            Iput {
                src: 14,
                obj: 15,
                field,
            },
            InvokeStaticRange {
                method,
                first_reg: 0,
                count: 16,
            },
            ReturnVoid,
        ],
    );
    let original = code.clone();
    let mut incoming = vec![dex.intern_type("I").expect("valid fixture descriptor"); 15];
    incoming.push(
        dex.intern_type("LExample;")
            .expect("valid fixture descriptor"),
    );
    assert!(grow_registers(&mut code, 3, &incoming, &[], &dex).is_err());
    assert_eq!(code, original);
}
