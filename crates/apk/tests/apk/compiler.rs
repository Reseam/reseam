// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::documents::*;
use crate::common::tables::*;
use reseam_apk::axml::{self, AxmlDocument, AxmlEvent};
use reseam_apk::{ResValue, ResourceScope};

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
