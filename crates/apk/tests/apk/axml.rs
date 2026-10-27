// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::documents::*;
use crate::common::table_bytes::*;
use crate::common::tables::*;
use reseam_apk::axml::{self, AxmlDocument, AxmlEvent};
use reseam_apk::{ResValue, ResourceScope, StringEncoding};

#[test]
fn manifest_edits_survive_both_string_encodings() -> TestResult {
    for encoding in [StringEncoding::Utf8, StringEncoding::Utf16] {
        let mut doc = make_test_axml(encoding);
        let original = doc.serialize()?;
        doc = AxmlDocument::parse(&original)?;
        assert_eq!(doc.serialize()?, original);
        assert!(doc.set_version_code(i32::MAX));
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
            assert_eq!(decoded.version_code(), Some(i32::MAX));
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
    let subtree = &source.events()[start..=axml::element_end(source.events(), start).expect("end")];
    assert!(
        missing
            .adopt(source.string_pool(), subtree, Some(&table))
            .is_err()
    );
    let adopted = target.adopt(source.string_pool(), subtree, Some(&table))?;
    target.insert_events(target.root().expect("root") + 1, adopted)?;
    let decoded = AxmlDocument::parse(&target.serialize()?)?;
    let graft = decoded.find_element("FrameLayout").expect("graft");
    assert_eq!(
        decoded
            .attribute(graft, attr)
            .expect("constraint")
            .namespace(),
        decoded.namespace_index(axml::APP_NS)
    );
    assert_eq!(
        attribute(&decoded, graft, "id").value().kind,
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
                .string(decoded.attributes(decoded.root().expect("root"))[index - 1].name())
                .as_deref(),
            Some(name)
        );
    }
    assert_eq!(
        &edited[chunk(&edited, 12..edited.len(), 0x104)],
        &original[chunk(&original, 12..original.len(), 0x104)]
    );
    let id = axml::android_attr_res_id("id").expect("ID");
    doc.edit_attributes(root, |document, attributes| {
        attributes.retain(|attribute| document.resource_id_for(attribute.name()) != Some(id));
    })
    .expect("root element");
    let removed = doc.serialize()?;
    let start = chunk(&removed, 12..removed.len(), 0x102).start;
    assert_eq!(u16_at(&removed, start + header + 14), 0);
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
fn framework_attributes_match_by_resource_id_when_their_name_string_is_scrambled() -> TestResult {
    let compiled = axml::compile_xml(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test"><application android:name=".App"/></manifest>"#,
        None,
    )?;
    let scrambled = replace_bytes(&compiled, b"name", b"zzzz");
    let doc = AxmlDocument::parse(&scrambled)?;
    let application = doc.find_element("application").expect("application");
    let attr = &doc.attributes(application)[0];
    assert_eq!(doc.string(attr.name()).as_deref(), Some("zzzz"));
    let android = doc.declared_namespace("android");
    assert!(doc.is_attribute(attr, android, "name"));
    assert!(!doc.is_attribute(attr, android, "label"));
    assert!(!doc.is_attribute(attr, None, "name"));
    assert_eq!(doc.application_name().as_deref(), Some(".App"));
    Ok(())
}

#[test]
fn documents_are_screened_for_attributes_by_resource_id_or_local_name() -> TestResult {
    let compiled = axml::compile_xml(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test"><application android:name=".App" plain="1"/></manifest>"#,
        None,
    )?;
    let scrambled = replace_bytes(&compiled, b"name", b"zzzz");
    for (data, name, expected) in [
        (&compiled, "android:name", true),
        (&scrambled, "android:name", true),
        (&compiled, "android:label", false),
        (&compiled, "plain", true),
        (&compiled, "plai", false),
    ] {
        assert_eq!(
            AxmlDocument::may_carry_attribute(data, &[name])?,
            expected,
            "{name}"
        );
    }
    Ok(())
}

fn replace_bytes(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let at = bytes
        .windows(from.len())
        .position(|window| window == from)
        .expect("string in pool");
    [&bytes[..at], to, &bytes[at + from.len()..]].concat()
}
