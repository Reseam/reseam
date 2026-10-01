// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::TypeIdx;
use crate::types::code::CodeItem;
use crate::types::instruction::Instruction;

use super::{
    find_contiguous_free_registers, find_free_register, find_free_registers, reaching_definitions,
};

fn code(instructions: Vec<Instruction>, registers_size: u16) -> CodeItem {
    CodeItem {
        registers_size,
        ins_size: 0,
        outs_size: 0,
        debug_info: None,
        instructions,
        tries: Vec::new(),
        catch_handlers: Vec::new(),
    }
}

#[test]
fn free_register_queries_respect_live_values_and_exclusions() {
    struct Case {
        instructions: Vec<Instruction>,
        size: u16,
        at: usize,
        excluded: Vec<u16>,
        first: u16,
        free: Vec<u16>,
        contiguous: Vec<u16>,
    }
    for case in [
        Case {
            instructions: vec![
                Instruction::Const { dest: 0, value: 1 },
                Instruction::AddInt {
                    dest: 1,
                    a: 0,
                    b: 2,
                },
                Instruction::Return { src: 1 },
            ],
            size: 5,
            at: 1,
            excluded: vec![],
            first: 1,
            free: vec![1, 3],
            contiguous: vec![3, 4],
        },
        Case {
            instructions: vec![
                Instruction::Const { dest: 0, value: 1 },
                Instruction::Return { src: 0 },
            ],
            size: 4,
            at: 1,
            excluded: vec![1],
            first: 2,
            free: vec![2, 3],
            contiguous: vec![2, 3],
        },
        Case {
            instructions: vec![
                Instruction::InvokeStaticRange {
                    method: crate::MethodIdx(0),
                    first_reg: 1,
                    count: 2,
                },
                Instruction::ReturnVoid,
            ],
            size: 6,
            at: 0,
            excluded: vec![],
            first: 0,
            free: vec![0, 3],
            contiguous: vec![3, 4],
        },
    ] {
        let code = code(case.instructions, case.size);
        assert_eq!(
            find_free_register(&code, case.at, &case.excluded),
            Some(case.first)
        );
        assert_eq!(
            find_free_registers(&code, case.at, 2, &case.excluded),
            Some(case.free)
        );
        assert_eq!(
            find_contiguous_free_registers(&code, case.at, 2, &case.excluded),
            Some(case.contiguous)
        );
    }
}

#[test]
fn reference_cast_refines_type_without_replacing_the_reaching_value() {
    let code = code(
        vec![
            Instruction::MoveObject { dest: 0, src: 1 },
            Instruction::CheckCast {
                ref_: 0,
                type_: TypeIdx(0),
            },
            Instruction::ReturnObject { src: 0 },
        ],
        2,
    );
    assert_eq!(reaching_definitions(&code, 2, 0), Some((vec![0], false)));
    assert_eq!(find_free_register(&code, 1, &[1]), None);
}

#[test]
fn keeps_both_words_of_incoming_wide_values_live() {
    let mut code = code(
        vec![
            Instruction::IputBoolean {
                src: 0,
                obj: 1,
                field: crate::FieldIdx(0),
            },
            Instruction::IputWide {
                src: 2,
                obj: 1,
                field: crate::FieldIdx(1),
            },
            Instruction::ReturnVoid,
        ],
        4,
    );
    code.ins_size = 4;
    assert_eq!(find_free_register(&code, 0, &[]), None);
    assert_eq!(find_free_registers(&code, 0, 1, &[]), None);
    assert_eq!(find_contiguous_free_registers(&code, 0, 1, &[]), None);
    assert_eq!(find_free_register(&code, 1, &[]), Some(0));
}

#[test]
fn follows_branches_and_back_edges_before_reusing_a_register() {
    let code = code(
        vec![
            Instruction::IfEqz { a: 0, offset: 4 },
            Instruction::Const16 { dest: 1, value: 0 },
            Instruction::Return { src: 1 },
        ],
        2,
    );
    assert_eq!(find_free_register(&code, 0, &[]), None);
    assert_eq!(find_free_register(&code, 1, &[]), Some(0));
    let code = super::tests::code(
        vec![
            Instruction::SputWide {
                src: 1,
                field: crate::FieldIdx(0),
            },
            Instruction::IfNez { a: 0, offset: -2 },
            Instruction::ReturnVoid,
        ],
        3,
    );
    assert_eq!(find_free_register(&code, 1, &[]), None);
}

#[test]
fn preserves_values_read_by_exception_handlers_before_a_write_completes() {
    let mut code = code(
        vec![
            Instruction::IgetWide {
                dest: 1,
                obj: 0,
                field: crate::FieldIdx(0),
            },
            Instruction::ReturnWide { src: 1 },
            Instruction::MoveException { dest: 0 },
            Instruction::ReturnWide { src: 1 },
        ],
        3,
    );
    code.tries.push(crate::TryItem {
        start_addr: 0,
        insn_count: 2,
        handler_idx: 0,
    });
    code.catch_handlers.push(crate::CatchHandler {
        typed_catches: vec![],
        catch_all_addr: Some(3),
    });
    assert_eq!(find_free_register(&code, 0, &[]), None);
}

#[test]
fn a_wide_write_to_the_register_below_defines_this_one() {
    let code = code(
        vec![
            Instruction::Const { dest: 1, value: 1 },
            Instruction::ConstWide16 { dest: 0, value: 7 },
            Instruction::Return { src: 1 },
        ],
        3,
    );
    assert_eq!(reaching_definitions(&code, 2, 1), Some((vec![1], false)));
    assert_eq!(reaching_definitions(&code, 0, 1), Some((vec![], true)));
}

#[test]
fn an_exception_edge_carries_the_definitions_reaching_the_thrower() {
    let mut code = code(
        vec![
            Instruction::Const4 { dest: 1, value: 1 },
            Instruction::IgetWide {
                dest: 1,
                obj: 0,
                field: crate::FieldIdx(0),
            },
            Instruction::ReturnWide { src: 1 },
            Instruction::MoveException { dest: 0 },
            Instruction::Return { src: 1 },
        ],
        3,
    );
    code.tries.push(crate::TryItem {
        start_addr: 1,
        insn_count: 2,
        handler_idx: 0,
    });
    code.catch_handlers.push(crate::CatchHandler {
        typed_catches: vec![],
        catch_all_addr: Some(4),
    });
    assert_eq!(reaching_definitions(&code, 2, 1), Some((vec![1], false)));
    assert_eq!(reaching_definitions(&code, 4, 1), Some((vec![0], false)));
}
