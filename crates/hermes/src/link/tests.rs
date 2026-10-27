// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;

use super::remap::literal_count;
use super::*;
use crate::FunctionId;
use crate::fixtures::{HOST, LITERALS, host, image, written};
use crate::literal::{Literal, read_literals};
use crate::opcode::{IdKind, Instruction};
use reseam_bytes::u32_at;

/// A literal as text and bytes, comparable across files whose string IDs differ.
#[derive(Debug, PartialEq)]
enum Value {
    Text(Vec<u16>),
    Other(Literal, Vec<u8>),
}

fn values(file: &HermesFile<'_>, buffer: Section, offset: u64, count: u32) -> Vec<Value> {
    read_literals(file, buffer, offset as usize, count as usize)
        .expect("fixture literals decode")
        .into_iter()
        .map(|literal| match literal.string() {
            Some(id) => Value::Text(
                file.string(id)
                    .expect("literal string exists")
                    .units()
                    .collect(),
            ),
            None => Value::Other(literal.kind, literal.bytes.to_vec()),
        })
        .collect()
}

fn operand(inst: &Instruction, kind: IdKind) -> Option<u64> {
    inst.operands()
        .find(|(operand, _)| operand.id == kind)
        .map(|(_, value)| value)
}

/// Offsets where a run starts when the buffer is read from its beginning.
fn run_starts(buffer: &[u8]) -> BTreeSet<u64> {
    let mut starts = BTreeSet::new();
    let mut cursor = 0;
    while cursor < buffer.len() {
        starts.insert(cursor as u64);
        let tag = buffer[cursor];
        let mut length = usize::from(tag & Literal::LENGTH);
        let mut header = 1;
        if tag & Literal::LONG_RUN != 0 {
            length = length << 8 | usize::from(buffer[cursor + 1]);
            header = 2;
        }
        cursor += header + length * Literal::decode(tag).size();
    }
    starts
}

#[test]
fn linked_literals_read_back_the_values_and_keys_of_the_module() {
    let module = image(LITERALS);
    let module_file = module.file();
    let host_functions = image(HOST).file().function_count();
    let mut editor = host();
    editor.link(&module_file, &[]).expect("module links");
    let linked = image(&written(&editor));
    let linked_file = linked.file();

    let source_shapes = module_file.section(Section::Shapes).as_chunks::<8>().0;
    let linked_shapes = linked_file.section(Section::Shapes).as_chunks::<8>().0;
    let starts = run_starts(module_file.section(Section::Values));
    let mut inside_a_run = false;
    for id in 0..module_file.function_count() {
        let source = module_file
            .function(FunctionId(id))
            .unwrap()
            .instructions()
            .unwrap();
        let copy = linked_file
            .function(FunctionId(host_functions + id))
            .unwrap()
            .instructions()
            .unwrap();
        for (source, copy) in source.iter().zip(&copy) {
            let Some(count) = literal_count(source, source_shapes, 0).unwrap() else {
                continue;
            };
            let (from, to) = (
                operand(source, IdKind::ValueBuffer).unwrap(),
                operand(copy, IdKind::ValueBuffer).unwrap(),
            );
            inside_a_run |= !starts.contains(&from);
            assert_eq!(
                values(&module_file, Section::Values, from, count),
                values(&linked_file, Section::Values, to, count),
                "values of {:?} in function {id}",
                source.op
            );
            if let (Some(from), Some(to)) =
                (operand(source, IdKind::Shape), operand(copy, IdKind::Shape))
            {
                let key = |shapes: &[[u8; 8]], index: u64| u32_at(&shapes[index as usize], 0);
                assert_eq!(
                    values(
                        &module_file,
                        Section::Keys,
                        u64::from(key(source_shapes, from)),
                        count
                    ),
                    values(
                        &linked_file,
                        Section::Keys,
                        u64::from(key(linked_shapes, to)),
                        count
                    ),
                    "keys of {:?} in function {id}",
                    source.op
                );
            }
        }
    }
    assert!(
        inside_a_run,
        "the fixture shares a literal sequence inside another run"
    );
}
