// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::{LAYOUT_PATH, entry, open_split_test_apk};
use crate::common::bundles::apply_fixture;
use reseam_apk::axml::AxmlAttribute;
use reseam_apk::{ResValue, ResourceTable};

#[test]
fn a_new_attribute_binds_the_id_the_inflater_resolves_it_by() {
    const TARGET_PACKAGE: u32 = 0x0101_0021;
    const TARGET_CLASS: u32 = 0x0101_002f;

    let (_apk_dir, mut apk) = open_split_test_apk();
    apply_fixture(&mut apk, "xml-attribute-binding", "xmlAttributeBinding");

    let manifest = apk.base().manifest();
    let intent = manifest.find_element("intent").unwrap();
    let value = |res_id| {
        manifest
            .attribute(intent, res_id)
            .and_then(|attr| manifest.attribute_string(attr))
            .map(std::borrow::Cow::into_owned)
    };
    assert_eq!(value(TARGET_PACKAGE).as_deref(), Some("com.example.target"));
    assert_eq!(
        value(TARGET_CLASS).as_deref(),
        Some("com.example.target.Settings")
    );
}

fn inline_file_paths(table: &ResourceTable, pulse: &reseam_apk::AxmlDocument) -> [String; 2] {
    let root = pulse.root().unwrap();
    let target = pulse.find_element("target").unwrap();
    [(root, "drawable"), (target, "animation")].map(|(element, attribute)| {
        let value = pulse.attribute_named(element, attribute).unwrap().value();
        assert_eq!(value.kind, ResValue::REFERENCE);
        let (_, file) = table.values(value.data).next().unwrap().unwrap();
        assert_eq!(file.kind, ResValue::STRING);
        table.get_string(file.data).unwrap().unwrap().into_owned()
    })
}

fn assert_resource_values(table: &ResourceTable) {
    const WINDOW_BACKGROUND: u32 = 0x0101_0054;
    const TEXT_COLOR: u32 = 0x0101_0098;
    let icon = table
        .find_resource_id("drawable", "reseam_icon")
        .unwrap()
        .unwrap();
    for ((kind, name), path) in [
        (("drawable", "reseam_icon"), "res/reseam_icon.png"),
        (("mipmap", "reseam_launcher"), "res/reseam_launcher.png"),
    ] {
        assert_eq!(table.file_paths(kind, name).unwrap(), [path]);
    }
    let items = |name: &str| {
        table
            .complex_entries("style", name)
            .unwrap()
            .unwrap_or_else(|| panic!("style/{name} is not a complex entry"))
            .into_iter()
            .map(|entry| (entry.name, entry.value))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        items("Theme.Test"),
        [
            (WINDOW_BACKGROUND, ResValue::reference(icon)),
            (
                TEXT_COLOR,
                ResValue::new(ResValue::INT_COLOR_ARGB8, 0xff10_2030)
            ),
        ],
        "the existing item was replaced, the new one added, and both kept in id order"
    );
    let app_attr = table
        .find_resource_id("attr", "layout_constraintRight_toLeftOf")
        .unwrap()
        .unwrap();
    assert_eq!(
        items("Theme.Reseam").first().map(|item| item.0),
        Some(app_attr)
    );
    let elements = table
        .complex_entries("array", "double_tap_lengths")
        .unwrap()
        .unwrap();
    assert!(
        elements[..7]
            .iter()
            .all(|entry| entry.value.kind == ResValue::STRING)
    );
    assert_eq!(
        elements[..7]
            .iter()
            .map(|entry| table
                .get_string(entry.value.data)
                .unwrap()
                .unwrap()
                .into_owned())
            .collect::<Vec<_>>(),
        [
            "5",
            "10",
            "15",
            "20",
            &format!("@0x{icon:08x}"),
            "true",
            "@string/missing"
        ]
    );
    let scalars: Vec<_> = elements[7..].iter().map(|entry| entry.value).collect();
    assert_eq!(
        scalars,
        [
            ResValue::new(ResValue::FLOAT, 0x8000_0000),
            ResValue::new(ResValue::DIMENSION, 0x0000_2101),
            ResValue::new(ResValue::INT_BOOLEAN, 1),
            ResValue::reference(icon)
        ]
    );
}

#[test]
fn a_typed_file_resource_a_style_and_an_array_land_where_the_loader_reads_them() {
    let (_apk_dir, mut apk) = open_split_test_apk();
    apply_fixture(&mut apk, "resource-entries", "resourceEntries");

    assert_eq!(
        apk.read_component_entry(0, "assets/resource-entries.txt")
            .unwrap(),
        Some(format!("{LAYOUT_PATH}|5,10").into_bytes()),
        "the layout resolved to its obfuscated path and the array read back as text"
    );
    assert!(
        apk.read_component_entry(0, "res/reseam_icon.png")
            .unwrap()
            .is_some()
    );
    let pulse_bytes = entry(&mut apk, 0, "res/drawable/reseam_pulse.xml").unwrap();
    let table = apk.component_mut(0).unwrap().resources().unwrap().unwrap();

    assert_resource_values(table);
    let pulse = reseam_apk::AxmlDocument::parse(&pulse_bytes).unwrap();
    let paths = inline_file_paths(table, &pulse);
    for (path, element) in paths.iter().zip(["vector", "objectAnimator"]) {
        let bytes = entry(&mut apk, 0, path).unwrap();
        let document = reseam_apk::AxmlDocument::parse(&bytes).unwrap();
        assert!(document.find_element(element).is_some());
    }
    let bytes = entry(&mut apk, 0, "res/split.xml").unwrap();
    let doc = reseam_apk::AxmlDocument::parse(&bytes).unwrap();
    assert_eq!(
        doc.element_name(doc.root().unwrap()).as_deref(),
        Some("base")
    );
    assert!(doc.find_element("split").is_none());
    let bytes = entry(&mut apk, 1, "res/split.xml").unwrap();
    let doc = reseam_apk::AxmlDocument::parse(&bytes).unwrap();
    assert!(doc.find_element("split").is_some());
}

#[test]
fn an_adopted_subtree_carries_the_ids_the_inflater_resolves_it_by() {
    const RES_AUTO: &str = "http://schemas.android.com/apk/res-auto";
    const ANDROID_ID: u32 = 0x0101_00d0;
    const ANDROID_SRC: u32 = 0x0101_0119;

    let (_apk_dir, mut apk) = open_split_test_apk();
    apply_fixture(&mut apk, "xml-graft", "xmlGraft");

    let layout = entry(&mut apk, 0, LAYOUT_PATH).unwrap();
    let layout = reseam_apk::AxmlDocument::parse(&layout).unwrap();
    let table = apk.component_mut(0).unwrap().resources().unwrap().unwrap();
    let app_attr = table
        .find_resource_id("attr", "layout_constraintRight_toLeftOf")
        .unwrap()
        .unwrap();
    let controls = table.find_resource_id("id", "controls").unwrap().unwrap();
    let fullscreen = table
        .find_resource_id("id", "fullscreen_button")
        .unwrap()
        .unwrap();
    let icon = table
        .find_resource_id("drawable", "reseam_icon")
        .unwrap()
        .unwrap();

    let container = layout.find_element("FrameLayout").unwrap();
    let res_auto = layout.namespace_index(RES_AUTO).unwrap();
    let constraint = layout.attribute(container, app_attr).unwrap();
    assert_eq!(constraint.value(), ResValue::reference(fullscreen));
    assert_eq!(
        constraint.namespace(),
        Some(res_auto),
        "the source prefix was yt, the target declares app, and both mean res-auto"
    );
    assert_eq!(
        layout
            .attribute(container, ANDROID_ID)
            .map(|attr| attr.value().kind),
        Some(ResValue::REFERENCE)
    );
    assert_eq!(
        layout.element_name(container).as_deref(),
        Some("FrameLayout")
    );
    assert!(
        layout.attribute_named(container, "ignore").is_none(),
        "tools attributes are build-time hints the compiler drops"
    );

    let image = (0..layout.events().len())
        .filter(|&i| layout.element_name(i).as_deref() == Some("ImageView"))
        .collect::<Vec<_>>();
    assert_eq!(image.len(), 2);
    assert_eq!(
        layout
            .attribute(image[1], ANDROID_SRC)
            .map(AxmlAttribute::value),
        Some(ResValue::reference(icon)),
        "the fragment compiled against the drawable the patch had just registered"
    );
    assert_eq!(
        layout
            .attribute(image[0], app_attr)
            .map(AxmlAttribute::value),
        Some(ResValue::reference(controls)),
        "set binds an app attribute on a document that declares res-auto"
    );

    let manifest = apk.base().manifest();
    assert!(
        manifest.namespace_index(RES_AUTO).is_some(),
        "declareNamespace added res-auto to a document that lacked it"
    );
    assert_eq!(
        manifest
            .attribute(manifest.root().unwrap(), app_attr)
            .map(AxmlAttribute::value),
        Some(ResValue::reference(controls))
    );
}
