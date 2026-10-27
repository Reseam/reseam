// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::{LAYOUT_PATH, open_dex_apk};
use crate::common::bundles::apply_fixture;
use crate::common::code::{patched_code, read_method, written_dex};
use crate::common::dex;
use reseam_dex::ParseOptions;
use reseam_patcher::context::{ExtensionSet, PatchContext};

#[test]
fn replace_strings_containing_or_starting_with_rewrites_only_what_transform_returns() {
    use reseam_dex::Instruction::ConstString;

    const REWRITTEN: &str = "content://app.reseam.gsf.gservices/prefix";
    const ACTION: &str = "com.google.android.gsf.intent.SYNC";

    let owner = "Lcom/example/StringHolder;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("embedded_strings")]);
    apply_fixture(&mut apk, "embedded-strings", "embeddedStrings");
    assert_eq!(
        apk.read_component_entry(0, "assets/embedded-strings.txt")
            .unwrap(),
        Some(b"2|1".to_vec()),
        "two URIs contain the value; only the action starts with it",
    );

    let dex = written_dex(&apk, 0);
    for (name, expected) in [("uris", vec![REWRITTEN, ACTION]), ("more", vec![REWRITTEN])] {
        let code = read_method(&dex, owner, name).unwrap().code.unwrap();
        let constants: Vec<_> = code
            .instructions()
            .iter()
            .filter_map(|instruction| match instruction {
                ConstString { string, .. } => Some(dex.strings().get(*string).into_owned()),
                _ => None,
            })
            .collect();
        assert_eq!(constants, expected);
    }
}

#[test]
fn enum_constants_resolve_by_name_through_obfuscated_fields() {
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("obfuscated_enum")]);
    apply_fixture(&mut apk, "enum-constants", "enumConstants");

    assert_eq!(
        apk.read_component_entry(0, "assets/enum-constants.txt")
            .unwrap(),
        Some(b"Lcom/example/Plan;|c|d".to_vec()),
        "R8 renamed PREMIUM and FAMILY to c and d; the class query finds the enum by its names"
    );
    let written = written_dex(&apk, 0);
    let code = read_method(&written, "Lcom/example/Account;", "plan")
        .unwrap()
        .code
        .unwrap();
    let read = code
        .instructions()
        .iter()
        .find_map(reseam_dex::Instruction::field_ref)
        .map(|idx| {
            written
                .strings()
                .get(written.field_id(idx).name)
                .into_owned()
        });
    assert_eq!(read.as_deref(), Some("c"));
}

#[test]
fn field_access_queries_find_readers_writers_and_the_one_field_a_method_reads() {
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("obfuscated_enum")]);
    apply_fixture(&mut apk, "field-accesses", "fieldAccesses");

    assert_eq!(
        apk.read_component_entry(0, "assets/field-accesses.txt")
            .unwrap(),
        Some(b"Lcom/example/Plan;.<clinit>|Lcom/example/Plan;.a|Lcom/example/Plan;.b".to_vec()),
    );
}

#[test]
fn xml_files_are_found_by_the_attributes_their_elements_carry() {
    let (_apk_dir, mut apk) = open_dex_apk(&[]);
    apply_fixture(&mut apk, "xml-files-with", "xmlFilesWith");

    assert_eq!(
        apk.read_component_entry(0, "assets/xml-files-with.txt")
            .unwrap(),
        Some(format!("{LAYOUT_PATH}|").into_bytes()),
    );
}

#[test]
fn enum_constants_resolve_when_optimizers_bypass_the_enum_constructor() {
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("optimized_enum")]);
    apply_fixture(
        &mut apk,
        "optimized-enum-constants",
        "optimizedEnumConstants",
    );

    assert_eq!(
        apk.read_component_entry(0, "assets/optimized-enum-constants.txt")
            .unwrap(),
        Some(b"Lcom/example/License;|c|Lcom/example/Tier;|c".to_vec()),
        "License constants call Enum.<init> directly; Tier constants are built by their bodies' subclasses"
    );
    let written = written_dex(&apk, 0);
    for (method, owner) in [
        ("license", "Lcom/example/License;"),
        ("tier", "Lcom/example/Tier;"),
    ] {
        let code = read_method(&written, "Lcom/example/Account;", method)
            .unwrap()
            .code
            .unwrap();
        let read = code
            .instructions()
            .iter()
            .find_map(reseam_dex::Instruction::field_ref)
            .map(|idx| {
                let field = written.field_id(idx);
                (
                    written
                        .strings()
                        .get(written.type_string(field.class))
                        .into_owned(),
                    written.strings().get(field.name).into_owned(),
                )
            });
        assert_eq!(read, Some((owner.to_string(), "c".to_string())), "{method}");
    }
}

#[test]
fn a_query_can_require_instruction_order_and_a_predicate_of_its_own() {
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("ordered_queries")]);
    apply_fixture(
        &mut apk,
        "instruction-shape-queries",
        "instructionShapeQueries",
    );

    assert_eq!(
        apk.read_component_entry(0, "assets/instruction-shape.txt")
            .unwrap(),
        Some(b"alpha|gamma|shown|Lcom/example/Binder;".to_vec()),
    );
}

#[test]
fn a_class_reaches_the_methods_its_base_declares() {
    const BASE: &str = "Lcom/example/PreferenceBase;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("inherited_app"), dex!("inherited_base")]);
    apply_fixture(&mut apk, "inherited-methods", "inheritedMethods");

    assert_eq!(
        apk.read_component_entry(0, "assets/inherited-methods.txt")
            .unwrap(),
        Some(format!("{BASE}|{BASE}").into_bytes()),
        "inClass(inherited) and a call site's callee both resolve to the base declaration",
    );
}

#[test]
fn string_prefix_queries_use_literal_prefixes_and_exclude_extensions() {
    let (_dir, mut apk) = open_dex_apk(&[dex!("prefix_queries")]);
    apk.add_dex(reseam_dex::parse(dex!("extension_routes"), ParseOptions::default()).unwrap());
    apply_fixture(&mut apk, "string-prefix-queries", "stringPrefixQueries");
}

#[test]
fn high16_literals_are_consistent_in_queries_and_mutations() {
    use reseam_dex::Instruction::*;

    let owner = "Lcom/example/High16;";

    let (_dir, mut apk) = open_dex_apk(&[dex!("high_literals")]);
    apply_fixture(&mut apk, "high16-literals", "high16Literals");
    assert!(matches!(
        patched_code(&apk, owner, "positive").instructions()[0],
        ConstHigh16 { value: 2048, .. }
    ));
    assert!(matches!(
        patched_code(&apk, owner, "negative").instructions()[0],
        ConstHigh16 { value: -2, .. }
    ));
    assert!(matches!(
        patched_code(&apk, owner, "wide").instructions()[0],
        ConstWideHigh16 {
            value: i16::MIN,
            ..
        }
    ));
}

#[test]
fn hierarchy_queries_follow_subclasses_across_dex_files_and_direct_interfaces() {
    use reseam_dex::AccessFlags;

    // PreferenceFragment, in the first DEX, extends PreferenceBase from the second.
    let (_dir, mut apk) = open_dex_apk(&[dex!("inherited_app"), dex!("inherited_base")]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let base = ctx.dex_file_mut(1).unwrap();
    let object = Some("Ljava/lang/Object;");
    let fragment = Some("Lcom/example/PreferenceFragment;");
    base.create_class("Lcom/example/DeepFragment;", AccessFlags::PUBLIC, fragment)
        .unwrap();
    base.create_class("Lcom/example/Listener;", AccessFlags::INTERFACE, object)
        .unwrap();
    let listener = base.intern_type("Lcom/example/Listener;").unwrap();
    let implementer = base
        .create_class("Lcom/example/Impl;", AccessFlags::PUBLIC, object)
        .unwrap();
    base.class_mut(implementer)
        .unwrap()
        .interfaces
        .push(listener);
    base.create_class(
        "Lcom/example/ImplChild;",
        AccessFlags::PUBLIC,
        Some("Lcom/example/Impl;"),
    )
    .unwrap();

    let names = |found: Vec<reseam_patcher::context::ClassLocation>| {
        let mut names: Vec<String> = found
            .iter()
            .map(|location| {
                let dex = ctx.dex_file(location.dex_idx).unwrap();
                let class_type = dex.classes().header(location.class_idx).class_type;
                dex.type_descriptor(class_type).into_owned()
            })
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        names(
            ctx.find_classes_extending("Lcom/example/PreferenceBase;")
                .unwrap()
        ),
        [
            "Lcom/example/DeepFragment;",
            "Lcom/example/PreferenceFragment;"
        ],
    );
    assert_eq!(
        names(
            ctx.find_classes_implementing("Lcom/example/Listener;")
                .unwrap()
        ),
        ["Lcom/example/Impl;"],
        "a subclass inherits the interface without declaring it",
    );
    assert!(ctx.find_classes_extending("LAbsent;").unwrap().is_empty());
}

#[test]
fn instance_field_queries_read_lazy_fields_and_observe_edits() {
    use reseam_dex::{AccessFlags, EncodedField};

    let (_dir, mut apk) = open_dex_apk(&[dex!("instance_fields")]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    assert_eq!(ctx.dex().materialization_stats().resolved_classes, 0);
    let matches = ctx
        .find_classes_with_instance_field("Ljava/lang/String;")
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(ctx.dex().materialization_stats().resolved_classes, 0);
    assert!(
        ctx.find_classes_with_instance_field("LAbsent;")
            .unwrap()
            .is_empty()
    );
    let dex = ctx.dex_file_mut(0).unwrap();
    let index = dex.find_class_index("LStatic;").unwrap();
    let field = dex
        .intern_field("LStatic;", "instance", "Ljava/lang/String;")
        .unwrap();
    dex.class_mut(index)
        .unwrap()
        .add_instance_field(EncodedField {
            field,
            access_flags: AccessFlags::PUBLIC,
        });
    assert_eq!(
        ctx.find_classes_with_instance_field("Ljava/lang/String;")
            .unwrap()
            .len(),
        2
    );
}
