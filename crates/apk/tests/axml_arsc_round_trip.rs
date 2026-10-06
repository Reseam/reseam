// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

#[path = "common/resources.rs"]
mod fixtures;
use fixtures::*;
#[path = "common/resource_cases.rs"]
mod resource_cases;
use reseam_apk::axml::{self, AxmlDocument, AxmlEvent};
use reseam_apk::resources::{AttrFormats, EntryValue, MapEntry, ResType, ResourceTable};
use reseam_apk::{ResValue, ResourceScope, StringEncoding};
use reseam_dex::file::DexBytes;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn manifest_edits_survive_both_string_encodings() -> TestResult {
    for encoding in [StringEncoding::Utf8, StringEncoding::Utf16] {
        let mut doc = make_test_axml(encoding);
        let original = doc.serialize()?;
        doc = AxmlDocument::parse(&original)?;
        assert_eq!(doc.serialize()?, original);
        assert!(doc.set_version_code(42));
        assert!(doc.add_permission("android.permission.INTERNET"));
        for text in [
            String::new(),
            "Latin & \"quotes\"".into(),
            "a".repeat(32_768),
            "🙂".repeat(8_192),
            format!("{}\0tail", "a".repeat(32_768)),
        ] {
            assert!(doc.set_version_name(&text));
            let decoded = AxmlDocument::parse(&doc.serialize()?)?;
            assert_eq!(decoded.version_name().as_deref(), Some(text.as_str()));
            assert_eq!(decoded.version_code(), Some(42));
            assert_eq!(decoded.package_name().as_deref(), Some("com.example.test"));
            let permission = decoded.find_element("uses-permission").expect("permission");
            assert_eq!(
                decoded
                    .attribute_string(attribute(&decoded, permission, "name"))
                    .as_deref(),
                Some("android.permission.INTERNET")
            );
        }
    }
    Ok(())
}

#[test]
fn compilation_rejects_unresolved_resources_and_unextracted_inline_attributes() {
    for xml in [
        "<a>",
        "<a/><b/>",
        "outside<a/>",
        "<!DOCTYPE a><a/>",
        "<p:a/>",
        "<a>&missing;</a>",
        r#"<View xmlns:android="http://schemas.android.com/apk/res/android" android:background="@android:color/notAColor"/>"#,
        r#"<View xmlns:android="http://schemas.android.com/apk/res/android" android:background="@drawable/missing"/>"#,
        r#"<View xmlns:aapt="http://schemas.android.com/aapt"><aapt:attr name="android:drawable"><vector/></aapt:attr></View>"#,
    ] {
        assert!(
            axml::build_document(xml, Some(&mut ResourceScope::from(&mut make_test_arsc())))
                .is_err(),
            "{xml}"
        );
    }
}

#[test]
fn test_xml_compiler_resolves_typed_resource_values() -> TestResult {
    for case in resource_cases::xml_cases()? {
        for (name, value) in case.values {
            assert_eq!(root_value(&case.doc, name), value);
        }
        for (name, kind) in case.kinds {
            assert_eq!(root_value(&case.doc, name).kind, kind);
        }
    }
    Ok(())
}

#[test]
fn an_adopted_subtree_is_rebound_in_the_document_that_takes_it() -> TestResult {
    let mut table = make_test_arsc();
    let attr = table
        .add_resource("attr", "constraint", ResValue::new(0, 0))?
        .expect("ID");
    let source = axml::build_document(
        r#"<FrameLayout xmlns:android="http://schemas.android.com/apk/res/android" xmlns:yt="http://schemas.android.com/apk/res-auto" android:id="@+id/reseam_button" yt:constraint="@string/hello"><ImageView android:tag="inner"/></FrameLayout>"#,
        Some(&mut ResourceScope::from(&mut table)),
    )?;
    let mut target = view(r#"android:tag="root""#, Some(&mut table));
    let mut missing = axml::build_document("<LinearLayout/>", None)?;
    let start = source.root().expect("root");
    let subtree = &source.events()[start..=source.find_end_element(start).expect("end")];
    assert!(missing.adopt(&source, subtree, Some(&table)).is_err());
    let adopted = target.adopt(&source, subtree, Some(&table))?;
    target.insert_events(target.root().expect("root") + 1, adopted)?;
    let decoded = AxmlDocument::parse(&target.serialize()?)?;
    let graft = decoded.find_element("FrameLayout").expect("graft");
    assert_eq!(
        decoded
            .attribute(graft, attr)
            .expect("constraint")
            .namespace,
        decoded.namespace_index(axml::APP_NS)
    );
    assert_eq!(
        attribute(&decoded, graft, "id").value.kind,
        ResValue::REFERENCE
    );
    let inner = decoded.find_element("ImageView").expect("child");
    assert_eq!(
        decoded
            .attribute_string(attribute(&decoded, inner, "tag"))
            .as_deref(),
        Some("inner")
    );
    Ok(())
}

#[test]
fn lazy_resource_reads_and_bag_edits_report_corruption() -> TestResult {
    for corrupt_pool in [false, true] {
        let mut bytes = table_with_configs(
            "array",
            "values",
            &[(
                vec![0; 48],
                EntryValue::Complex {
                    parent: 0,
                    entries: vec![MapEntry {
                        name: 0x0100_0001,
                        value: ResValue::string(0),
                    }],
                },
            )],
            &["original"],
        )
        .serialize()?;
        let offset = if corrupt_pool {
            chunk(&bytes, 12..bytes.len(), 1).start + 28
        } else {
            let pkg = chunk(&bytes, 12..bytes.len(), 0x200);
            let t = chunk(&bytes, pkg.start + 288..pkg.end, 0x201);
            t.start + u32_at(&bytes, t.start + 16) as usize + 12
        };
        bytes[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut table = ResourceTable::parse(DexBytes::from_vec(bytes))?;
        if corrupt_pool {
            assert!(table.get_string(0).is_err());
            assert!(table.array("values").is_err());
        } else {
            assert!(table.array_values("values").is_err());
            let before = table.serialize()?;
            assert!(
                table
                    .set_array_values("values", &[ResValue::int(5)])
                    .is_err()
            );
            assert_eq!(table.serialize()?, before);
        }
    }
    Ok(())
}

#[test]
fn compilation_reuses_resource_ids_from_splits() -> TestResult {
    let mut table = make_test_arsc();
    let local = table.ensure_id("local")?;
    let mut lookup =
        |kind: &str, name: &str| Ok((kind == "id" && name == "split").then_some(0x7f03_0007));
    {
        let mut scope = ResourceScope::new(&mut table, &mut lookup);
        for (name, expected) in [("local", local), ("split", Some(0x7f03_0007))] {
            assert_eq!(scope.ensure_id(name)?, expected);
        }
        let doc = axml::build_document(
            r#"<View xmlns:android="http://schemas.android.com/apk/res/android" android:id="@+id/split"/>"#,
            Some(&mut scope),
        )?;
        assert_eq!(root_value(&doc, "id"), ResValue::reference(0x7f03_0007));
        assert!(scope.ensure_id("new")?.is_some());
    }
    assert_eq!(table.find_resource_id("id", "split")?, None);
    assert!(table.find_resource_id("id", "new")?.is_some());
    Ok(())
}

#[test]
fn staged_resource_aliases_resolve_ids_without_rewriting_metadata() -> TestResult {
    for header in [12, 16] {
        let bytes = alias_resource_table(header, 2);
        let table = ResourceTable::parse(DexBytes::from_slice(&bytes))?;
        for (id, expected) in [
            (0x7fff_0000, 0x7f01_0000),
            (0x7ffe_0000, 0x7f01_0001),
            (0x7f01_0000, 0x7f01_0000),
        ] {
            assert_eq!(table.finalized_resource_id(id)?, expected);
        }
        assert_eq!(table.serialize()?, bytes);
    }
    for bytes in [extended_resource_table(), alias_resource_table(12, 3)] {
        assert!(
            ResourceTable::parse(DexBytes::from_vec(bytes))?
                .finalized_resource_id(0x7fff_0000)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn binary_xml_edits_retain_metadata_and_relocate_special_attribute_indices() -> TestResult {
    let original = extended_axml();
    let mut doc = AxmlDocument::parse(&original)?;
    assert_eq!(doc.serialize()?, original);
    let root = doc.root().expect("root");
    doc.set_attribute(
        root,
        axml::android_attr_res_id("enabled").expect("ID"),
        ResValue::boolean(false),
    );
    let added = doc.make_string_attribute("name", 0x0101_0003, "changed");
    assert!(doc.add_attribute(root, added));
    let edited = doc.serialize()?;
    let start = chunk(&edited, 12..edited.len(), 0x102).start;
    let header = usize::from(u16_at(&edited, start + 2));
    assert_eq!(u32_at(&edited, start + 8), 37);
    assert_eq!(&edited[start + 16..start + 20], &[0xde, 0xad, 0xbe, 0xef]);
    let decoded = AxmlDocument::parse(&edited)?;
    for (name, offset) in [("id", 14), ("class", 16), ("style", 18)] {
        let index = usize::from(u16_at(&edited, start + header + offset));
        assert_eq!(
            decoded
                .string(decoded.attributes(decoded.root().expect("root"))[index - 1].name)
                .as_deref(),
            Some(name)
        );
    }
    assert_eq!(
        &edited[chunk(&edited, 12..edited.len(), 0x104)],
        &original[chunk(&original, 12..original.len(), 0x104)]
    );
    assert!(doc.remove_attribute(root, axml::android_attr_res_id("id").expect("ID")));
    let removed = doc.serialize()?;
    let start = chunk(&removed, 12..removed.len(), 0x102).start;
    assert_eq!(u16_at(&removed, start + header + 14), 0);
    Ok(())
}

#[test]
fn compilation_and_editing_keep_namespace_identity_under_shadowing() -> TestResult {
    let mut table = make_test_arsc();
    let app_title = table
        .add_resource("attr", "title", ResValue::new(0, 0))?
        .expect("ID");
    let mut doc = axml::build_document(
        r#"<root xmlns:x="http://schemas.android.com/apk/res/android"><x:View x:title="framework"/><x:View xmlns:x="http://schemas.android.com/apk/res-auto" x:title="application"><child xmlns="urn:widgets"/></x:View><x:View x:title="restored"/></root>"#,
        Some(&mut ResourceScope::from(&mut table)),
    )?;
    let view = (0..doc.events().len())
        .filter(|&i| doc.element_name(i).as_deref() == Some("View"))
        .nth(1)
        .expect("shadowed View");
    let (_, bound) = doc.bind_attribute_name_at(view, "x:title", Some(&table))?;
    assert_eq!(doc.resource_id_for(bound), Some(app_title));
    let decoded = AxmlDocument::parse(&doc.serialize()?)?;
    let views: Vec<_> = (0..decoded.events().len())
        .filter(|&i| decoded.element_name(i).as_deref() == Some("View"))
        .collect();
    for (index, id, value) in [
        (
            views[0],
            axml::android_attr_res_id("title").expect("ID"),
            "framework",
        ),
        (views[1], app_title, "application"),
        (
            views[2],
            axml::android_attr_res_id("title").expect("ID"),
            "restored",
        ),
    ] {
        assert_eq!(
            decoded
                .attribute_string(decoded.attribute(index, id).expect("title"))
                .as_deref(),
            Some(value)
        );
    }
    let AxmlEvent::StartElement {
        namespace: Some(uri),
        ..
    } = &decoded.events()[decoded.find_element("child").expect("child")]
    else {
        panic!("namespace");
    };
    assert_eq!(decoded.string(*uri).as_deref(), Some("urn:widgets"));
    Ok(())
}

#[test]
fn inline_resource_compilation_registers_dependencies_and_preserves_text() -> TestResult {
    for (xml, expected) in [
        (
            r#"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt"><aapt:attr name="android:drawable"><vector android:width="24dp"><path android:fillColor="@android:color/white"/></vector></aapt:attr></animated-vector>"#,
            vec![],
        ),
        (
            r#"<a xmlns:aapt="http://schemas.android.com/aapt" xmlns:android="http://schemas.android.com/apk/res/android"><aapt:attr name="android:drawable"><b><aapt:attr name="android:src"><c>hello &amp; world </c></aapt:attr></b></aapt:attr></a>"#,
            vec!["hello & world "],
        ),
    ] {
        let mut table = make_test_arsc();
        let files = axml::compile_resource_file(
            xml,
            "res/drawable/icon.xml",
            "drawable",
            "icon",
            "",
            &mut ResourceScope::from(&mut table),
        )?;
        let main = AxmlDocument::parse(&files.last().expect("main").data)?;
        let drawable = root_value(&main, "drawable");
        assert_eq!(drawable.kind, ResValue::REFERENCE);
        for item in table.values(drawable.data) {
            assert!(files.iter().any(|f| {
                Some(f.path.as_str())
                    == table
                        .get_string(item.as_ref().expect("value").1.data)
                        .expect("path")
                        .as_deref()
            }));
        }
        let mut texts = Vec::new();
        for file in files {
            let name = file
                .path
                .rsplit('/')
                .next()
                .expect("name")
                .trim_end_matches(".xml");
            assert_eq!(
                table.file_paths("drawable", name)?,
                std::slice::from_ref(&file.path)
            );
            let doc = AxmlDocument::parse(&file.data)?;
            texts.extend(doc.events().iter().filter_map(|e| {
                if let AxmlEvent::Text { text, .. } = e {
                    Some(doc.string(*text).expect("text").into_owned())
                } else {
                    None
                }
            }));
        }
        assert_eq!(texts, expected);
    }
    Ok(())
}

#[test]
fn attribute_formats_control_compilation_and_style_literals() -> TestResult {
    for (formats, text, kind) in resource_cases::LITERALS {
        let mut table = attribute_table(&[("value", formats.bits())]);
        let doc = view(&format!(r#"app:value="{text}""#), Some(&mut table));
        let value = root_value(&doc, "value");
        assert_eq!(value.kind, kind, "{text}");
        if kind == ResValue::STRING {
            assert_eq!(doc.string(value.data).as_deref(), Some(text));
        }
        ResourceScope::from(&mut table).set_style_items(
            "Theme",
            Some("@0x01030005"),
            &[("value".into(), text.into())],
        )?;
        let value = table.complex_entries("style", "Theme")?.expect("bag")[0].value;
        assert_eq!(value.kind, kind);
        if kind == ResValue::STRING {
            assert_eq!(table.get_string(value.data)?.as_deref(), Some(text));
        }
    }
    for (formats, text) in [
        (AttrFormats::BOOLEAN, "12"),
        (AttrFormats::INTEGER, "true"),
        (AttrFormats::COLOR, "#éA"),
        (AttrFormats::DIMENSION, "-2147483648dp"),
    ] {
        let mut table = attribute_table(&[("value", formats.bits())]);
        let id = table.find_resource_id("attr", "value")?.expect("ID");
        assert!(
            axml::parse_attribute_value(text, id, Some(&mut ResourceScope::from(&mut table)))
                .is_err()
        );
    }
    for text in ["true", "12", "#abc"] {
        let doc = view(&format!(r#"android:contentDescription="{text}""#), None);
        let value = root_value(&doc, "contentDescription");
        assert_eq!(value.kind, ResValue::STRING);
        assert_eq!(doc.string(value.data).as_deref(), Some(text));
    }
    Ok(())
}

#[test]
fn fractional_dimension_values_retain_precision_and_units() {
    for (text, expected, unit) in [
        ("0.5dp", 0.5, 1),
        ("0.001sp", 0.001, 2),
        ("-1.25px", -1.25, 0),
        ("8388607dp", 8_388_607.0, 1),
    ] {
        let doc = view(&format!(r#"android:padding="{text}""#), None);
        let data = root_value(&doc, "padding").data;
        let value = f64::from(data as i32 >> 8)
            / [1.0, 128.0, 32_768.0, 8_388_608.0][((data >> 4) & 3) as usize];
        assert!((value - expected).abs() < 0.000_001, "{text}");
        assert_eq!(data & 15, unit);
    }
}

#[test]
fn resource_edits_reject_invalid_indices_without_changing_the_table() -> TestResult {
    let mut entries = ResType::new(1, vec![0; 48]);
    entries.push(Some(simple(0, ResValue::STRING, 0)))?;
    for index in [65_536, usize::MAX] {
        assert!(entries.set(index, None).is_err());
        assert_eq!(entries.len(), 1);
        assert!(entries.entry(index)?.is_none());
        assert_eq!(entries.entry(0)?.expect("entry").key, 0);
    }
    let mut table = table_with_configs(
        "array",
        "items",
        &[(
            vec![0; 48],
            EntryValue::Complex {
                parent: 0,
                entries: Vec::new(),
            },
        )],
        &[],
    );
    let original = table.serialize()?;
    assert!(
        table
            .set_array_values("items", &[ResValue::string(u32::MAX)])
            .is_err()
    );
    assert_eq!(table.serialize()?, original);
    Ok(())
}

#[test]
fn structural_xml_edits_require_a_complete_parent_and_preserve_child_order() -> TestResult {
    let mut doc = axml::build_document("<tree><existing/></tree>", None)?;
    let original = doc.serialize()?;
    let end = doc
        .events()
        .iter()
        .position(|e| matches!(e, AxmlEvent::EndElement { .. }))
        .expect("end");
    for parent in [usize::MAX, doc.events().len(), end] {
        assert!(
            doc.insert_child_element(parent, "invalid", Vec::new())
                .is_err()
        );
        assert!(!doc.append_child_element(parent, "invalid", Vec::new()));
        assert_eq!(doc.serialize()?, original);
    }
    let root = doc.root().expect("root");
    doc.insert_child_element(root, "first", Vec::new())?;
    assert!(doc.append_child_element(root, "last", Vec::new()));
    let decoded = AxmlDocument::parse(&doc.serialize()?)?;
    assert_eq!(
        (0..decoded.events().len())
            .filter_map(|i| decoded.element_name(i))
            .collect::<Vec<_>>(),
        ["tree", "first", "existing", "last"]
    );
    Ok(())
}

#[test]
fn resource_lookup_and_string_replacement_survive_serialization() -> TestResult {
    for (index, case) in resource_cases::cases()?.into_iter().enumerate() {
        let mut table = ResourceTable::parse(DexBytes::from_slice(&case.input))?;
        assert_eq!(table.serialize()?, case.input, "unchanged case {index}");
        (case.edit)(&mut table)?;
        assert_eq!(
            (case.read)(&round_trip(&table))?,
            case.expected,
            "edited case {index}"
        );
    }
    Ok(())
}
