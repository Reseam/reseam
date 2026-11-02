// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use reseam_dex::{SmaliLine, disassemble};
use reseam_journal::Savepoint;

use super::*;
use crate::bridge::bytecode::{code, replace_operands};
use crate::bridge::dex::types::{CodeInstruction, CodePlacement, CodeStep};
use crate::bridge::{savepoints, trace};
use crate::options::PatchOptions;
use crate::patch::PatchRef;

const NOP: Instruction = Instruction::Simple(SimpleInsn { opcode: 0x00 });
const RETURN_VOID: Instruction = Instruction::Simple(SimpleInsn { opcode: 0x0e });

/// Runs `edit` as the phase of patch `test/<name>`.
fn as_patch<R>(
    context: &mut PatchContext<'_>,
    directory: &Path,
    name: &str,
    edit: impl FnOnce() -> R,
) -> R {
    context.begin_patch(
        &PatchRef::parse(&format!("test/{name}")).unwrap(),
        PatchOptions::default(),
    );
    let guard = ContextGuard::enter(context, RunningBundle::at(directory.to_owned())).unwrap();
    let result = edit();
    guard.finish().unwrap();
    result
}

fn method(class: &str, proto: &str, ins: u16, instructions: Vec<Instruction>) -> u32 {
    let class = fixture_class(0, class);
    methods::add_method(
        class,
        NewMethod {
            instructions,
            ..fixture_method("run".into(), proto.into(), ins, ins)
        },
    )
}

fn smali(context: &mut PatchContext<'_>, method: u32) -> Vec<String> {
    let location = crate::bridge::handles::method_location(method).unwrap();
    let (dex, encoded) = context.read_method(location).unwrap().unwrap();
    disassemble(dex, encoded.code.as_ref().unwrap())
        .unwrap()
        .into_iter()
        .map(|line| match line {
            SmaliLine::Label(text)
            | SmaliLine::Directive(text)
            | SmaliLine::Instruction { text, .. } => text,
        })
        .collect()
}

fn log_call(message: &str) -> [String; 3] {
    [
        "const-string v0, \"reseam-trace\"".into(),
        format!("const-string v1, \"{message}\""),
        "invoke-static {v0, v1}, Landroid/util/Log;->i(Ljava/lang/String;Ljava/lang/String;)I"
            .into(),
    ]
}

#[test]
fn points_follow_later_edits_and_rollbacks() {
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
    let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    trace::start();
    let directory = directory.path();
    let method = as_patch(&mut context, directory, "setup", || {
        method("LEdits;", "()V", 0, vec![NOP, NOP, RETURN_VOID])
    });
    as_patch(&mut context, directory, "a", || {
        mutation::insert_instructions(method, 1, vec![NOP]).unwrap();
    });
    // Before a's code: branches to it skip b's, so a's point stays on a's code.
    as_patch(&mut context, directory, "b", || {
        mutation::insert_instructions(method, 1, vec![NOP]).unwrap();
    });
    let savepoint = Savepoint::FIRST;
    context.open_savepoint(savepoint);
    savepoints::open_savepoint(savepoint).unwrap();
    as_patch(&mut context, directory, "undone", || {
        mutation::insert_instructions(method, 0, vec![NOP, NOP]).unwrap();
    });
    context.rollback_to(savepoint);
    savepoints::rollback_to(savepoint).unwrap();
    as_patch(&mut context, directory, "c", || {
        mutation::remove_instructions(method, 3, 1).unwrap();
    });
    // Removes b's code, and b's point with it.
    as_patch(&mut context, directory, "d", || {
        mutation::remove_instructions(method, 1, 1).unwrap();
    });
    trace::insert(&mut context).unwrap();

    let expected: Vec<String> = [
        &["nop".into()][..],
        &log_call("test/a  LEdits;->run()V  [1]"),
        &log_call("test/d  LEdits;->run()V  [1]"),
        &["nop".into()],
        &log_call("test/c  LEdits;->run()V  [2]"),
        &["return-void".into()],
    ]
    .concat();
    assert_eq!(smali(&mut context, method), expected);
}

#[test]
fn code_before_returns_and_rewritten_operands_each_log() {
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
    let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    trace::start();
    let directory = directory.path();
    let method = as_patch(&mut context, directory, "setup", || {
        method(
            "LReturns;",
            "(I)V",
            1,
            vec![
                // if-eqz p0, +4 (the second return); const/4 p0, #1; return-void; return-void
                Instruction::Raw(vec![0x38, 0x00, 0x04, 0x00]),
                Instruction::Raw(vec![0x12, 0x10]),
                RETURN_VOID,
                RETURN_VOID,
            ],
        )
    });
    as_patch(&mut context, directory, "returns", || {
        let nop = CodeStep::Instruction(CodeInstruction {
            template: NOP,
            operands: Vec::new(),
            target: None,
        });
        code::emit_code(method, CodePlacement::BeforeReturns, vec![nop], Vec::new()).unwrap();
    });
    as_patch(&mut context, directory, "literal", || {
        assert_eq!(replace_operands::replace_literals(method, 1, 2), 1);
    });
    trace::insert(&mut context).unwrap();

    let message =
        |patch: &str, index: usize| format!("test/{patch}  LReturns;->run(I)V  [{index}]");
    let expected: Vec<String> = [
        &["if-eqz p0, :cond_0".into()][..],
        &log_call(&message("literal", 1)),
        &["const/4 p0, 0x2".into()],
        &log_call(&message("returns", 2)),
        &["nop".into(), "return-void".into(), ":cond_0".into()],
        &log_call(&message("returns", 4)),
        &["nop".into(), "return-void".into()],
    ]
    .concat();
    assert_eq!(smali(&mut context, method), expected);
}

#[test]
fn code_written_at_the_end_logs_where_it_starts() {
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
    let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    trace::start();
    let directory = directory.path();
    let (replaced, appended) = as_patch(&mut context, directory, "setup", || {
        (
            method("LReplaced;", "()V", 0, vec![NOP, RETURN_VOID]),
            method("LAppended;", "()V", 0, vec![NOP, RETURN_VOID]),
        )
    });
    as_patch(&mut context, directory, "body", || {
        let ret = CodeStep::Instruction(CodeInstruction {
            template: RETURN_VOID,
            operands: Vec::new(),
            target: None,
        });
        code::emit_code(replaced, CodePlacement::Body, vec![ret], Vec::new()).unwrap();
    });
    as_patch(&mut context, directory, "append", || {
        mutation::insert_instructions(appended, 2, vec![RETURN_VOID]).unwrap();
    });
    trace::insert(&mut context).unwrap();

    let expected: Vec<String> = [
        log_call("test/body  LReplaced;->run()V  [0]").as_slice(),
        &["return-void".into()],
    ]
    .concat();
    assert_eq!(smali(&mut context, replaced), expected);
    let expected: Vec<String> = [
        &["nop".into(), "return-void".into()][..],
        &log_call("test/append  LAppended;->run()V  [2]"),
        &["return-void".into()],
    ]
    .concat();
    assert_eq!(smali(&mut context, appended), expected);
}
