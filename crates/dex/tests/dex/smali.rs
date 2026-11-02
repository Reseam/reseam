// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::{self, TestResult};
use reseam_dex::{
    CatchHandler, CodeItem, Instruction, Loading, PackedSwitchData, SmaliLine, TryItem, TypedCatch,
    disassemble,
};

fn text(lines: &[SmaliLine]) -> Vec<&str> {
    lines
        .iter()
        .map(|line| match line {
            SmaliLine::Label(text)
            | SmaliLine::Directive(text)
            | SmaliLine::Instruction { text, .. } => text.as_str(),
        })
        .collect()
}

#[test]
fn bodies_print_as_baksmali_prints_them_with_sequential_labels() -> TestResult {
    let mut dex = common::dex();
    let string = dex.intern_string("it's \"é\"");
    let exception = dex.intern_type("Ljava/lang/Exception;")?;
    let mut code = CodeItem::new(
        3,
        1,
        0,
        vec![
            Instruction::Const4 { dest: 0, value: 1 },
            Instruction::IfEqz { a: 2, offset: 5 },
            Instruction::ConstString { dest: 1, string },
            Instruction::Goto { offset: 9 },
            Instruction::ConstWide {
                dest: 0,
                value: 0x1_0000_0000,
            },
            Instruction::PackedSwitch {
                test: 0,
                payload_offset: 5,
            },
            Instruction::ReturnVoid,
            Instruction::ReturnVoid,
            Instruction::PackedSwitchPayload(Box::new(PackedSwitchData {
                first_key: 0,
                targets: vec![3, 4],
            })),
        ],
    )?;
    code.set_exception_handlers(
        vec![TryItem {
            start_addr: 3,
            insn_count: 2,
            handler_idx: 0,
        }],
        vec![CatchHandler {
            typed_catches: vec![TypedCatch {
                exception_type: exception,
                addr: 15,
            }],
            catch_all_addr: Some(14),
        }],
    )?;

    let lines = disassemble(&dex, &code)?;

    assert_eq!(
        text(&lines),
        [
            "const/4 v0, 0x1",
            "if-eqz p0, :cond_0",
            ":try_start_0",
            r#"const-string v1, "it\'s \"\u00e9\"""#,
            ":try_end_0",
            ".catch Ljava/lang/Exception; {:try_start_0 .. :try_end_0} :catch_0",
            ".catchall {:try_start_0 .. :try_end_0} :catchall_0",
            "goto :goto_0",
            ":cond_0",
            "const-wide v0, 0x100000000L",
            "packed-switch v0, :pswitch_data_0",
            ":catchall_0",
            ":goto_0",
            ":pswitch_0",
            "return-void",
            ":catch_0",
            ":pswitch_1",
            "return-void",
            ":pswitch_data_0",
            ".packed-switch 0x0\n    :pswitch_0\n    :pswitch_1\n.end packed-switch",
        ]
    );
    assert!(lines.contains(&SmaliLine::Instruction {
        index: 4,
        text: "const-wide v0, 0x100000000L".to_owned()
    }));
    Ok(())
}

#[test]
fn reading_a_class_leaves_it_in_the_file() -> TestResult {
    let bytes = common::write(&common::dex())?;
    let dex = common::parse_with_loading(&bytes, Loading::Deferred)?;

    let class = dex.classes().read(0, dex.parse_options())?;

    assert_eq!(dex.type_descriptor(class.class_type), "Lexample/Activity;");
    assert_eq!(
        class
            .class_data
            .as_ref()
            .map(|data| data.direct_methods.len()),
        Some(1)
    );
    assert!(!dex.classes().is_resident(0));
    Ok(())
}
