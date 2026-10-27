// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::HermesError;
use crate::code::{BlockId, Code, Condition, HookKind, Operand, Place, SlotId, Statement};
use crate::error::Unsupported;
use crate::fixtures::{BUNDLED, LITERALS, function, host, image, written};

/// Code that reads every kind of operand, branches and calls an export.
fn exercising(module: crate::ModuleId, kind: HookKind) -> Code {
    let result = if kind == HookKind::After {
        Operand::Result
    } else {
        Operand::Param(0)
    };
    let call = |into| Statement::Call {
        into: SlotId(into),
        module,
        export: "withProperty".into(),
        arguments: vec![
            Operand::Text("bundle".into()),
            Operand::Int(-1),
            Operand::Bool(true),
            Operand::Receiver,
            Operand::Param(2),
        ],
    };
    Code {
        blocks: vec![
            vec![
                call(0),
                Statement::Branch {
                    condition: Condition::NotNull(Operand::Slot(SlotId(0))),
                    then: BlockId(1),
                    otherwise: BlockId(2),
                },
                Statement::Assign {
                    place: Place::Param(1),
                    value: result,
                },
            ],
            vec![Statement::Return(Operand::Slot(SlotId(0)))],
            vec![
                call(1),
                Statement::Branch {
                    condition: Condition::Equal(Operand::Slot(SlotId(1)), Operand::Null),
                    then: BlockId(3),
                    otherwise: BlockId(4),
                },
            ],
            vec![Statement::Return(Operand::Undefined)],
            vec![],
        ],
    }
}

#[test]
fn rollback_returns_the_bundle_to_each_savepoint() {
    use reseam_journal::Savepoint;

    let module = image(BUNDLED);
    let mut editor = host();
    let greet = function(&editor, "greet");
    let original = written(&editor);

    let outer = Savepoint::FIRST;
    editor.open_savepoint(outer);
    let id = editor.link(&module.file(), &[]).expect("module links");
    editor
        .hook(greet, HookKind::Before, &exercising(id, HookKind::Before))
        .expect("hook applies");
    let after_outer = written(&editor);

    let inner = outer.next();
    editor.open_savepoint(inner);
    for kind in [HookKind::After, HookKind::Replace] {
        editor
            .hook(greet, kind, &exercising(id, kind))
            .expect("hook applies");
    }
    editor
        .link(&module.file(), &[])
        .expect("module links again");

    editor.rollback_to(inner);
    assert_eq!(written(&editor), after_outer);
    editor.rollback_to(outer);
    assert_eq!(written(&editor), original);
    assert!(!editor.is_edited());
}

#[test]
fn hooked_functions_jump_only_to_their_own_instructions() {
    let module = image(BUNDLED);
    let mut editor = host();
    let greet = function(&editor, "greet");
    let id = editor.link(&module.file(), &[]).expect("module links");
    for kind in [HookKind::Before, HookKind::After, HookKind::Replace] {
        editor
            .hook(greet, kind, &exercising(id, kind))
            .unwrap_or_else(|error| panic!("{kind:?} hook applies: {error}"));
    }
    let hooked = image(&written(&editor));
    let file = hooked.file();
    for id in 0..file.function_count() {
        let function = file
            .function(crate::FunctionId(id))
            .expect("function parses");
        let code = function.instructions().expect("function decodes");
        let starts: std::collections::BTreeSet<i64> = code
            .iter()
            .map(|inst| i64::from(inst.offset.expect("decoded")))
            .collect();
        let end = function.body_range().len() as i64;
        for inst in &code {
            for (_, relative) in inst.jumps() {
                let target = i64::from(inst.offset.expect("decoded")) + relative;
                assert!(
                    starts.contains(&target) || target == end,
                    "function {id} jumps from {:?} to byte {target}",
                    inst.op
                );
            }
        }
    }
}

fn calling(module: crate::ModuleId, export: &str) -> Code {
    Code {
        blocks: vec![vec![Statement::Call {
            into: SlotId(0),
            module,
            export: export.into(),
            arguments: vec![Operand::Param(0)],
        }]],
    }
}

#[test]
fn hooks_reach_the_exports_link_returns() {
    let greet = function(&host(), "greet");
    let modules: [(&[u8], &[&str]); 2] = [
        (LITERALS, &["rows", "render"]),
        (BUNDLED, &["isEnabled", "withProperty"]),
    ];
    for (bytes, exports) in modules {
        let module = image(bytes);
        let mut editor = host();
        let id = editor.link(&module.file(), &[]).expect("module links");
        for export in exports {
            editor
                .hook(greet, HookKind::Before, &calling(id, export))
                .unwrap_or_else(|error| panic!("{export} is exported: {error}"));
        }
        assert!(matches!(
            editor.hook(greet, HookKind::Before, &calling(id, "missing")),
            Err(HermesError::Unsupported {
                reason: Unsupported::Export(_),
                ..
            })
        ));
    }
}
