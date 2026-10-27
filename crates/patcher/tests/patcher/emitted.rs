// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::open_dex_apk;
use crate::common::bundles::apply_fixture;
use crate::common::code::{hook_calls, invoked_by, patched_code, read_method, written_dex};
use crate::common::dex;

#[test]
fn emitted_equality_preserves_numeric_values_and_reference_identity() {
    let owner = "Lcom/example/ComparisonHost;";

    let (_directory, mut apk) = open_dex_apk(&[dex!("comparisons")]);
    apply_fixture(&mut apk, "typed-comparisons", "typedComparisons");
    let wide_words = |bits: u64| [i64::from(bits as u32), i64::from((bits >> 32) as u32)];
    for (kind, cases) in [
        (
            "long",
            vec![
                ((0_u64, 0_u64), true),
                ((1, 1 << 32), false),
                ((i64::MIN as u64, i64::MAX as u64), false),
                ((u64::MAX, u64::MAX), true),
            ],
        ),
        (
            "float",
            vec![
                ((0, u64::from((-0.0_f32).to_bits())), true),
                (
                    (u64::from(f32::NAN.to_bits()), u64::from(f32::NAN.to_bits())),
                    false,
                ),
                (
                    (u64::from(1.0_f32.to_bits()), u64::from(2.0_f32.to_bits())),
                    false,
                ),
            ],
        ),
        (
            "double",
            vec![
                ((0, (-0.0_f64).to_bits()), true),
                ((f64::NAN.to_bits(), f64::NAN.to_bits()), false),
                ((1.0_f64.to_bits(), 2.0_f64.to_bits()), false),
            ],
        ),
        (
            "int",
            vec![((0, 0), true), ((42, 42), true), ((1, 2), false)],
        ),
        (
            "reference",
            vec![
                ((0, 0), true),
                ((101, 101), true),
                ((101, 202), false),
                ((0, 101), false),
            ],
        ),
    ] {
        for ((a, b), equal) in cases {
            let incoming = if kind == "long" || kind == "double" {
                [wide_words(a), wide_words(b)].concat()
            } else {
                vec![a as i64, b as i64]
            };
            for (suffix, expected) in [("Equal", equal), ("NotEqual", !equal)] {
                let code = patched_code(&apk, owner, &format!("{kind}{suffix}"));
                assert_eq!(
                    hook_calls(&code, &incoming),
                    (vec![], Some(i64::from(expected))),
                    "{kind}{suffix}({a}, {b})"
                );
            }
        }
    }
}

#[test]
fn emitted_arguments_accept_unresolved_hierarchies_and_reject_proven_mismatches() {
    let owner = "Lcom/example/TypeHost;";

    let (_directory, mut apk) = open_dex_apk(&[dex!("types")]);
    apply_fixture(&mut apk, "incomplete-hierarchies", "incompleteHierarchies");
    for name in [
        "application",
        "image",
        "appView",
        "libraryActivity",
        "missingSource",
        "missingTarget",
        "knownSubclass",
    ] {
        assert_eq!(
            hook_calls(&patched_code(&apk, owner, name), &[123]),
            (vec![vec![123]], None),
            "{name}"
        );
    }
    for name in [
        "unrelated",
        "primitiveReference",
        "wideNarrow",
        "arrayElements",
    ] {
        let code = patched_code(&apk, owner, name);
        assert_eq!(
            hook_calls(&code, &[123, 456][..usize::from(code.ins_size())]),
            (vec![], None),
            "{name}"
        );
    }
    assert_eq!(
        apk.read_component_entry(0, "assets/type-validation.txt")
            .unwrap(),
        Some(b"true|true|true|true".to_vec())
    );
}

#[test]
fn emitted_constants_preserve_values_across_encoding_widths_and_branches() {
    let owner = "Lcom/example/ConstantHost;";

    let cases = [
        ("small", 7),
        ("shortMax", 32767),
        ("shortMin", -32768),
        ("positiveWide", 32768),
        ("negativeWide", -32769),
        ("positiveHigh", 65536),
        ("negativeHigh", -65536),
        ("color", -16_777_216),
        ("minimum", i32::MIN),
        ("maximum", i32::MAX),
    ];

    let (_directory, mut apk) = open_dex_apk(&[dex!("constants")]);
    apply_fixture(&mut apk, "constant-forms", "constantForms");
    for (name, value) in cases {
        let code = patched_code(&apk, owner, name);
        for (condition, expected) in [(0, 0), (1, value)] {
            assert_eq!(
                hook_calls(&code, &[condition]),
                (vec![], Some(i64::from(expected))),
                "{name}({condition})"
            );
        }
    }
}

#[test]
fn a_goto_past_127_code_units_lands_on_its_label() {
    let (_directory, mut apk) = open_dex_apk(&[dex!("constants")]);
    apply_fixture(&mut apk, "long-goto", "longGoto");
    let code = patched_code(&apk, "Lcom/example/ConstantHost;", "small");
    assert_eq!(hook_calls(&code, &[1]), (vec![], Some(0)));
}

#[test]
fn emitted_operands_reach_registers_their_encoding_cannot_name() {
    let (_directory, mut apk) = open_dex_apk(&[dex!("entry_values")]);
    apply_fixture(&mut apk, "distant-operands", "distantOperands");
    let code = patched_code(&apk, "Lcom/example/HookTarget;", "invokeGrowth");
    assert!(code.registers_size() - code.ins_size() > 15);
    for (incoming, entered) in [([1, 2, 6, 6, 9], true), ([1, 2, 6, 7, 9], false)] {
        let (calls, _) = hook_calls(&code, &incoming);
        assert_eq!(calls.first() == Some(&vec![9]), entered, "{incoming:?}");
    }
}

#[test]
fn an_added_method_gets_its_body_and_can_call_its_superclass() {
    use reseam_dex::{AccessFlags, Instruction};

    let (_directory, mut apk) = open_dex_apk(&[dex!("entry_app"), dex!("entry_base")]);
    apply_fixture(&mut apk, "added-method", "addedMethod");
    let dex = (0..2)
        .map(|index| written_dex(&apk, index))
        .find(|dex| dex.find_class_index("Lcom/example/App;").is_some())
        .unwrap();
    let method = read_method(&dex, "Lcom/example/App;", "reseamCallsBase").unwrap();
    assert_eq!(
        method.access_flags,
        AccessFlags::PUBLIC | AccessFlags::FINAL
    );
    let code = method.code.as_deref().unwrap();
    assert!(
        code.instructions()
            .iter()
            .any(|instruction| matches!(instruction, Instruction::InvokeSuper { .. }))
    );
    assert_eq!(
        invoked_by(&dex, "Lcom/example/App;", "reseamCallsBase"),
        ["Lcom/example/AppBase;->onCreate"]
    );
}
