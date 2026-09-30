// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_apk::axml::{self, AxmlAttribute, AxmlDocument, AxmlEvent};
use reseam_apk::resources::{
    config_for_qualifiers, EntryValue, MapEntry, ResEntry, ResPackage, ResType, ResourceTable,
    TypeSpec,
};
use reseam_apk::{ResValue, ResourceScope, StringPool};
use reseam_dex::file::DexBytes;

fn make_test_axml(is_utf8: bool) -> AxmlDocument {
    let strings = [
        "http://schemas.android.com/apk/res/android",
        "android",
        "manifest",
        "package",
        "versionCode",
        "versionName",
        "com.example.test",
        "1.0.0",
    ];
    let elements = vec![
        AxmlEvent::StartNamespace {
            prefix: Some(1),
            uri: 0,
        },
        AxmlEvent::StartElement {
            namespace: None,
            name: 2,
            attributes: vec![
                AxmlAttribute::new(None, 3, ResValue::string(6)),
                AxmlAttribute::new(Some(0), 4, ResValue::int(1)),
                AxmlAttribute::new(Some(0), 5, ResValue::string(7)),
            ],
        },
        AxmlEvent::EndElement {
            namespace: None,
            name: 2,
        },
        AxmlEvent::EndNamespace {
            prefix: Some(1),
            uri: 0,
        },
    ];
    AxmlDocument {
        string_pool: StringPool::new(strings.iter().map(|s| s.to_string()).collect(), is_utf8),
        resource_ids: vec![0, 0, 0, 0, 0x0101_021b, 0x0101_021c],
        elements,
    }
}

fn assert_same_strings(a: &AxmlDocument, b: &AxmlDocument) {
    assert_eq!(a.string_pool.len(), b.string_pool.len());
    for (i, (x, y)) in a.string_pool.iter().zip(b.string_pool.iter()).enumerate() {
        assert_eq!(x, y, "string {i} mismatch");
    }
}

#[test]
fn test_axml_round_trip_synthetic() {
    let doc = make_test_axml(true);
    let bytes = doc.serialize().expect("serialize failed");
    let reparsed = AxmlDocument::parse(&bytes).expect("reparse failed");

    assert_same_strings(&doc, &reparsed);
    assert_eq!(doc.resource_ids, reparsed.resource_ids);
    assert_eq!(doc.elements.len(), reparsed.elements.len());
    assert_eq!(reparsed.package_name().as_deref(), Some("com.example.test"));
    assert_eq!(reparsed.version_code(), Some(1));
    assert_eq!(reparsed.version_name().as_deref(), Some("1.0.0"));
}

#[test]
fn test_axml_round_trip_utf16() {
    let doc = make_test_axml(false);
    let bytes = doc.serialize().expect("serialize failed");
    let reparsed = AxmlDocument::parse(&bytes).expect("reparse failed");

    assert!(!reparsed.string_pool.is_utf8());
    assert_same_strings(&doc, &reparsed);
    assert_eq!(reparsed.package_name().as_deref(), Some("com.example.test"));
}

#[test]
fn test_axml_round_trip_mutated() {
    let mut doc = make_test_axml(true);
    assert!(doc.set_version_code(42));
    assert!(doc.set_version_name("2.0.0"));

    let bytes = doc.serialize().expect("serialize failed");
    let reparsed = AxmlDocument::parse(&bytes).expect("reparse failed");

    assert_eq!(reparsed.version_code(), Some(42));
    assert_eq!(reparsed.version_name().as_deref(), Some("2.0.0"));
    assert_eq!(reparsed.package_name().as_deref(), Some("com.example.test"));
}

#[test]
fn test_axml_round_trip_add_permission() {
    let mut doc = make_test_axml(true);
    let original_element_count = doc.elements.len();
    assert!(doc.add_permission("android.permission.INTERNET"));

    let bytes = doc.serialize().expect("serialize failed");
    let reparsed = AxmlDocument::parse(&bytes).expect("reparse failed");

    assert_eq!(reparsed.elements.len(), original_element_count + 2);
    let permission = reparsed
        .find_element("uses-permission")
        .expect("permission element");
    let name = reparsed
        .attribute(permission, 0x0101_0003)
        .expect("name attribute");
    assert_eq!(
        reparsed.attribute_string(name).as_deref(),
        Some("android.permission.INTERNET")
    );
    assert_eq!(reparsed.package_name().as_deref(), Some("com.example.test"));
}

fn strings(values: &[&str]) -> StringPool {
    StringPool::new(values.iter().map(|s| s.to_string()).collect(), true)
}

fn simple(key: u32, kind: u8, data: u32) -> Option<ResEntry> {
    Some(ResEntry {
        flags: 0,
        key,
        value: EntryValue::Simple(ResValue::new(kind, data)),
    })
}

fn simple_value(entry: &ResEntry) -> ResValue {
    match entry.value {
        EntryValue::Simple(value) => value,
        EntryValue::Complex { .. } => panic!("expected simple value"),
    }
}

fn make_test_arsc() -> ResourceTable {
    let mut pkg = ResPackage::new(
        0x7F,
        "com.example.test",
        strings(&["string"]),
        strings(&["hello", "world"]),
    );
    pkg.type_specs.push(TypeSpec::new(1, vec![0, 0]));
    let mut t = ResType::new(1, vec![0; 48]);
    t.push(simple(0, ResValue::STRING, 0));
    t.push(simple(1, ResValue::STRING, 1));
    pkg.types.push(t);
    ResourceTable {
        global_strings: strings(&["Hello", "World", "app_name"]),
        packages: vec![pkg],
    }
}

#[test]
fn test_find_resource_id_across_packages() {
    let mut table = make_test_arsc();
    table.packages.insert(
        0,
        ResPackage::new(0x7E, "empty.pkg", strings(&[]), strings(&[])),
    );

    assert_eq!(table.find_resource_id("string", "hello"), Some(0x7F01_0000));
}

#[test]
fn test_xml_compiler_resolves_typed_resource_values() {
    let mut table = make_test_arsc();
    let string_id = table
        .find_resource_id("string", "hello")
        .expect("string id");
    let local_attr_id = table
        .add_resource("attr", "titleText", ResValue::new(0, 0))
        .expect("attr id");
    let android_attr_id = axml::android_attr_res_id("textColor").expect("android attr id");
    let xml = r#"
        <TextView
            xmlns:android="http://schemas.android.com/apk/res/android"
            android:text="@string/hello"
            android:id="@+id/title"
            android:theme="?attr/titleText"
            android:textColor="?android:attr/textColor"
            android:background="@android:color/white"
            android:padding="16dp"
            android:alpha="0.5" />
    "#;

    let doc =
        axml::build_document(xml, Some(&mut ResourceScope::from(&mut table))).expect("build axml");
    let element = doc.root().expect("start element");
    let attr = |name: &str| {
        doc.attribute_named(element, name)
            .expect("attribute present")
            .value
    };

    assert_eq!(attr("text"), ResValue::reference(string_id));
    assert_eq!(
        attr("id"),
        ResValue::reference(table.find_resource_id("id", "title").unwrap())
    );
    assert_eq!(
        table.resource_value("id", "title"),
        Some(ResValue::id_entry())
    );
    assert_eq!(attr("theme"), ResValue::attribute(local_attr_id));
    assert_eq!(attr("textColor"), ResValue::attribute(android_attr_id));
    assert_eq!(
        attr("background"),
        ResValue::reference(axml::android_res_id("color", "white").unwrap())
    );
    assert_eq!(attr("padding").kind, ResValue::DIMENSION);
    assert_eq!(attr("alpha").kind, ResValue::FLOAT);
}

#[test]
fn test_xml_compiler_rejects_inline_resources() {
    let xml = r#"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt"><aapt:attr name="android:drawable"><vector /></aapt:attr></animated-vector>"#;

    let err = axml::build_document(xml, None).expect_err("inline resource");

    assert!(err.to_string().contains("ResourceScope.addFile"), "{err}");
}

#[test]
fn test_xml_compiler_rejects_unknown_framework_resources() {
    let mut table = make_test_arsc();
    let xml = r#"<View xmlns:android="http://schemas.android.com/apk/res/android" android:background="@android:color/notAColor" />"#;

    let err = axml::build_document(xml, Some(&mut ResourceScope::from(&mut table)))
        .expect_err("unknown framework color");

    assert!(err.to_string().contains("android:color/notAColor"), "{err}");
}

/// `attr/mode` (enum: compact, wide) and `attr/sides` (flags: top, bottom),
/// with their names as `id` entries, the way aapt compiles `<attr>`.
fn make_attr_arsc() -> ResourceTable {
    const ATTR_TYPE: u32 = 0x0100_0000;
    let mut pkg = ResPackage::new(
        0x7F,
        "com.example.attrs",
        strings(&["attr", "id"]),
        strings(&["mode", "sides", "compact", "wide", "top", "bottom"]),
    );
    let bag = |key: u32, format: u32, symbols: [(u32, u32); 2]| {
        Some(ResEntry {
            flags: 0x0001,
            key,
            value: EntryValue::Complex {
                parent: 0,
                entries: std::iter::once(MapEntry {
                    name: ATTR_TYPE,
                    value: ResValue::int(format as i32),
                })
                .chain(symbols.map(|(name, value)| MapEntry {
                    name,
                    value: ResValue::int(value as i32),
                }))
                .collect(),
            },
        })
    };
    pkg.type_specs.push(TypeSpec::new(1, vec![0, 0]));
    let mut attrs = ResType::new(1, vec![0; 48]);
    attrs.push(bag(0, 1 << 16, [(0x7F02_0000, 1), (0x7F02_0001, 2)]));
    attrs.push(bag(1, 1 << 17, [(0x7F02_0002, 1), (0x7F02_0003, 2)]));
    pkg.types.push(attrs);
    pkg.type_specs.push(TypeSpec::new(2, vec![0; 4]));
    let mut ids = ResType::new(2, vec![0; 48]);
    for key in 2..6 {
        ids.push(simple(key, ResValue::INT_BOOLEAN, 0));
    }
    pkg.types.push(ids);
    ResourceTable {
        global_strings: strings(&[]),
        packages: vec![pkg],
    }
}

#[test]
fn test_xml_compiler_rejects_undefined_references() {
    let mut table = make_test_arsc();
    let xml = r#"
        <layer-list xmlns:android="http://schemas.android.com/apk/res/android">
            <item android:drawable="@drawable/missing" />
        </layer-list>
    "#;

    let error = axml::build_document(xml, Some(&mut ResourceScope::from(&mut table)))
        .expect_err("undefined reference");
    assert!(error.to_string().contains("@drawable/missing"), "{error}");
}

#[test]
fn test_xml_compiler_resolves_enum_and_flag_names() {
    let mut table = make_attr_arsc();
    let gravity = |name| {
        axml::android_attr_symbol(axml::android_attr_res_id("gravity").unwrap(), name)
            .unwrap()
            .value
    };
    let xml = r#"
        <ImageView
            xmlns:android="http://schemas.android.com/apk/res/android"
            xmlns:app="http://schemas.android.com/apk/res-auto"
            android:scaleType="center"
            android:gravity="center_vertical | end"
            android:contentDescription="center"
            app:mode="wide"
            app:sides="top|bottom" />
    "#;

    let doc =
        axml::build_document(xml, Some(&mut ResourceScope::from(&mut table))).expect("build axml");
    let element = doc.root().expect("start element");
    let attr = |name: &str| {
        doc.attribute_named(element, name)
            .expect("attribute present")
            .value
    };

    assert_eq!(attr("scaleType"), ResValue::int(5));
    assert_eq!(
        attr("gravity"),
        ResValue::hex(gravity("center_vertical") | gravity("end"))
    );
    assert_eq!(attr("contentDescription").kind, ResValue::STRING);
    assert_eq!(attr("mode"), ResValue::int(2));
    assert_eq!(attr("sides"), ResValue::hex(3));
}

#[test]
fn test_arsc_round_trip_synthetic() {
    let table = make_test_arsc();
    let bytes = table.serialize().expect("serialize failed");
    let reparsed = ResourceTable::parse(DexBytes::from_vec(bytes)).expect("reparse failed");

    assert_eq!(
        table.global_strings.iter().collect::<Vec<_>>(),
        reparsed.global_strings.iter().collect::<Vec<_>>()
    );
    assert_eq!(table.packages.len(), reparsed.packages.len());

    let pkg = &reparsed.packages[0];
    assert_eq!(pkg.id, 0x7F);
    assert_eq!(pkg.name, "com.example.test");
    assert_eq!(pkg.type_strings.iter().collect::<Vec<_>>(), ["string"]);
    assert_eq!(
        pkg.key_strings.iter().collect::<Vec<_>>(),
        ["hello", "world"]
    );
    assert_eq!(pkg.type_specs.len(), 1);
    assert_eq!(pkg.types.len(), 1);

    let t = &pkg.types[0];
    assert_eq!(t.id, 1);
    assert_eq!(t.len(), 2);
    assert!(t.entry(1).is_some());
    let e0 = t.entry(0).unwrap();
    assert_eq!(e0.key, 0);
    assert_eq!(simple_value(&e0), ResValue::string(0));
}

#[test]
fn test_arsc_round_trip_complex_entries() {
    let mut pkg = ResPackage::new(
        0x7F,
        "com.example",
        strings(&["style"]),
        strings(&["AppTheme"]),
    );
    pkg.type_specs.push(TypeSpec::new(1, vec![0]));
    let mut t = ResType::new(1, vec![0; 48]);
    t.push(Some(ResEntry {
        flags: 0x0001,
        key: 0,
        value: EntryValue::Complex {
            parent: 0x01030005,
            entries: vec![
                MapEntry {
                    name: 0x010100D4,
                    value: ResValue::reference(0x7F020001),
                },
                MapEntry {
                    name: 0x010100D5,
                    value: ResValue::reference(0x7F020002),
                },
            ],
        },
    }));
    pkg.types.push(t);
    let table = ResourceTable {
        global_strings: strings(&["test"]),
        packages: vec![pkg],
    };

    let bytes = table.serialize().expect("serialize failed");
    let reparsed = ResourceTable::parse(DexBytes::from_vec(bytes)).expect("reparse failed");

    let entry = reparsed.packages[0].types[0].entry(0).unwrap();
    let EntryValue::Complex { parent, entries } = &entry.value else {
        panic!("expected complex value");
    };
    assert_eq!(*parent, 0x01030005);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, 0x010100D4);
    assert_eq!(entries[0].value, ResValue::reference(0x7F020001));
    assert_eq!(entries[1].name, 0x010100D5);
    assert_eq!(entries[1].value, ResValue::reference(0x7F020002));
}

#[test]
fn test_arsc_round_trip_mutated() {
    let mut table = make_test_arsc();
    table.set_string(0, "Modified".to_string());

    let refs = table.find_entries_by_string(1);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].key_name, "world");
    table.replace_entry_string(refs[0].res_id, 0);

    let bytes = table.serialize().expect("serialize failed");
    let reparsed = ResourceTable::parse(DexBytes::from_vec(bytes)).expect("reparse failed");

    assert_eq!(reparsed.global_strings.get(0).as_deref(), Some("Modified"));
    let entry = reparsed.packages[0].types[0].entry(1).unwrap();
    assert_eq!(simple_value(&entry).data, 0);
}

#[test]
fn test_arsc_round_trip_with_none_entries() {
    let mut table = make_test_arsc();
    table.packages[0].types[0].push(None);
    table.packages[0].types[0].push(None);
    table.packages[0].type_specs[0].push(0);
    table.packages[0].type_specs[0].push(0);

    let bytes = table.serialize().expect("serialize failed");
    let reparsed = ResourceTable::parse(DexBytes::from_vec(bytes)).expect("reparse failed");

    let t = &reparsed.packages[0].types[0];
    assert_eq!(t.len(), 4);
    assert!(t.entry(0).is_some());
    assert!(t.entry(1).is_some());
    assert!(t.entry(2).is_none());
    assert!(t.entry(3).is_none());
}

const YOUTUBE_APK: &str = "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";
const INSTAGRAM_APK: &str = "../../test-apks/com.instagram.android_419.0.0.49.71-382508603_minAPI28(arm64-v8a)(360,400,420,480dpi)_apkmirror.com.apk";

fn available_apks() -> Vec<&'static str> {
    [YOUTUBE_APK, INSTAGRAM_APK]
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect()
}

fn read_entry(apk_path: &str, name: &str) -> Vec<u8> {
    let file = std::fs::File::open(apk_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut entry = archive.by_name(name).unwrap();
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut buf).unwrap();
    buf
}

#[test]
fn test_axml_round_trip_real_apks() {
    let apks = available_apks();
    if apks.is_empty() {
        return;
    }

    for apk_path in &apks {
        let doc = AxmlDocument::parse(&read_entry(apk_path, "AndroidManifest.xml"))
            .expect("parse failed");
        let serialized = doc.serialize().expect("serialize failed");
        let reparsed = AxmlDocument::parse(&serialized).expect("reparse failed");

        assert_same_strings(&doc, &reparsed);
        assert_eq!(doc.resource_ids, reparsed.resource_ids);
        assert_eq!(doc.elements.len(), reparsed.elements.len());
        assert_eq!(doc.package_name(), reparsed.package_name());
        assert_eq!(doc.version_code(), reparsed.version_code());
        assert_eq!(doc.version_name(), reparsed.version_name());
    }
}

#[test]
fn test_arsc_round_trip_real_apks() {
    let apks = available_apks();
    if apks.is_empty() {
        return;
    }

    for apk_path in &apks {
        let arsc_bytes = read_entry(apk_path, "resources.arsc");
        let table = ResourceTable::parse(DexBytes::from_vec(arsc_bytes)).expect("parse failed");
        let serialized = table.serialize().expect("serialize failed");
        let reparsed =
            ResourceTable::parse(DexBytes::from_vec(serialized)).expect("reparse failed");

        assert_eq!(table.global_strings.len(), reparsed.global_strings.len());
        assert_eq!(table.packages.len(), reparsed.packages.len());
        for (i, (orig, re)) in table.packages.iter().zip(&reparsed.packages).enumerate() {
            assert_eq!(orig.id, re.id, "package {i} id mismatch");
            assert_eq!(orig.name, re.name, "package {i} name mismatch");
            assert_eq!(
                orig.type_strings.len(),
                re.type_strings.len(),
                "package {i} type_strings count mismatch"
            );
            assert_eq!(
                orig.key_strings.len(),
                re.key_strings.len(),
                "package {i} key_strings count mismatch"
            );
            assert_eq!(
                orig.type_specs.len(),
                re.type_specs.len(),
                "package {i} type_specs count mismatch"
            );
            assert_eq!(
                orig.types.len(),
                re.types.len(),
                "package {i} types count mismatch"
            );
        }
    }
}

#[test]
fn test_arsc_mutation_preserves_real_type_header_sizes() {
    let apks = available_apks();
    if apks.is_empty() {
        return;
    }

    for apk_path in &apks {
        let arsc_bytes = read_entry(apk_path, "resources.arsc");
        let original_headers = collect_type_header_sizes(&arsc_bytes).expect("header scan failed");
        let mut table =
            ResourceTable::parse(DexBytes::from_slice(&arsc_bytes)).expect("parse failed");
        table.add_global_string("reseam mutation sentinel");
        let serialized = table.serialize().expect("serialize failed");
        let mutated_headers =
            collect_type_header_sizes(&serialized).expect("mutated header scan failed");
        assert_eq!(
            original_headers, mutated_headers,
            "type header sizes changed after string-pool mutation for {apk_path}"
        );
    }
}

fn collect_type_header_sizes(bytes: &[u8]) -> Result<Vec<u16>, String> {
    fn read_u16(data: &[u8], offset: usize) -> Result<u16, String> {
        data.get(offset..offset + 2)
            .and_then(|slice| slice.try_into().ok())
            .map(u16::from_le_bytes)
            .ok_or_else(|| format!("short read at {offset}"))
    }

    fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
        data.get(offset..offset + 4)
            .and_then(|slice| slice.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| format!("short read at {offset}"))
    }

    let mut headers = Vec::new();
    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let chunk_type = read_u16(bytes, pos)?;
        let chunk_size = read_u32(bytes, pos + 4)? as usize;
        if chunk_size < 8 || pos + chunk_size > bytes.len() {
            return Err(format!("invalid chunk size at {pos}"));
        }
        if chunk_type == 0x0200 {
            let package = &bytes[pos..pos + chunk_size];
            let mut ppos = 288usize;
            while ppos + 8 <= package.len() {
                let pkg_chunk_type = read_u16(package, ppos)?;
                let pkg_header_size = read_u16(package, ppos + 2)?;
                let pkg_chunk_size = read_u32(package, ppos + 4)? as usize;
                if pkg_chunk_size < 8 || ppos + pkg_chunk_size > package.len() {
                    return Err(format!("invalid package chunk size at {ppos}"));
                }
                if pkg_chunk_type == 0x0201 {
                    headers.push(pkg_header_size);
                }
                ppos += pkg_chunk_size;
            }
        }
        pos += chunk_size;
    }
    Ok(headers)
}

/// A package with one type, `type_name`, whose entry `entry_name` is defined in
/// each of `configs`.
fn table_with_configs(
    type_name: &str,
    entry_name: &str,
    configs: &[(Vec<u8>, EntryValue)],
    global: &[&str],
) -> ResourceTable {
    let mut pkg = ResPackage::new(
        0x7F,
        "com.example",
        strings(&[type_name]),
        strings(&[entry_name]),
    );
    pkg.type_specs.push(TypeSpec::new(1, vec![0]));
    for (config, value) in configs {
        let mut res_type = ResType::new(1, config.clone());
        res_type.push(Some(ResEntry {
            flags: 0,
            key: 0,
            value: value.clone(),
        }));
        pkg.types.push(res_type);
    }
    ResourceTable {
        global_strings: strings(global),
        packages: vec![pkg],
    }
}

fn density_config(density: u16) -> Vec<u8> {
    let mut config = vec![0u8; 48];
    config[14..16].copy_from_slice(&density.to_le_bytes());
    config
}

#[test]
fn a_file_resource_reads_back_as_a_path_per_configuration() {
    let table = table_with_configs(
        "drawable",
        "splash",
        &[
            (density_config(480), EntryValue::Simple(ResValue::string(1))),
            (vec![0u8; 48], EntryValue::Simple(ResValue::string(0))),
        ],
        &["res/a.xml", "res/b.xml"],
    );

    // The default configuration comes first whatever order the chunks are in.
    assert_eq!(
        table.file_paths("drawable", "splash").unwrap(),
        ["res/a.xml", "res/b.xml"]
    );
    assert!(table
        .file_paths("drawable", "missing")
        .unwrap_err()
        .to_string()
        .contains("the table has no drawable/missing"));

    let table = table_with_configs(
        "bool",
        "flag",
        &[(vec![0u8; 48], EntryValue::Simple(ResValue::boolean(true)))],
        &[],
    );
    assert!(table
        .file_paths("bool", "flag")
        .unwrap_err()
        .to_string()
        .contains("is not file-backed"));
}

#[test]
fn an_entry_of_a_type_with_no_default_configuration_makes_one() {
    let mut table = table_with_configs(
        "mipmap",
        "launcher",
        &[(density_config(480), EntryValue::Simple(ResValue::string(0)))],
        &["res/launcher.png"],
    );

    let id = table
        .add_file_resource("mipmap", "reseam_launcher", "res/reseam.png", "")
        .expect("register the file");

    // The density chunk already owns index 0, so the new entry takes the next
    // one and every chunk of the type grows to cover it.
    assert_eq!(id & 0xFFFF, 1);
    assert_eq!(
        table.file_paths("mipmap", "reseam_launcher").unwrap(),
        ["res/reseam.png"]
    );
    assert_eq!(
        table.file_paths("mipmap", "launcher").unwrap(),
        ["res/launcher.png"]
    );
    let package = &table.packages[0];
    assert_eq!(package.type_specs[0].len(), 2);
    assert!(package.types.iter().all(|res_type| res_type.len() == 2));
    // An entry written into the density chunk instead would be invisible at
    // every other density, which is the failure this guards.
    let holder = package
        .types
        .iter()
        .find(|res_type| res_type.entry(1).is_some())
        .expect("the new entry");
    assert!(holder.is_default_config());

    let reparsed = ResourceTable::parse(DexBytes::from_vec(table.serialize().unwrap())).unwrap();
    assert_eq!(
        reparsed.file_paths("mipmap", "reseam_launcher").unwrap(),
        ["res/reseam.png"]
    );
}

#[test]
fn a_file_resource_lands_in_the_configuration_its_qualifiers_name() {
    let mut table = table_with_configs(
        "mipmap",
        "launcher",
        &[(vec![0u8; 48], EntryValue::Simple(ResValue::string(0)))],
        &["res/launcher.png"],
    );

    let hdpi = table
        .add_file_resource("mipmap", "reseam_icon", "res/hdpi.png", "hdpi")
        .unwrap();
    let xxhdpi = table
        .add_file_resource("mipmap", "reseam_icon", "res/xxhdpi.png", "xxhdpi")
        .unwrap();
    let anydpi = table
        .add_file_resource("mipmap", "reseam_icon", "res/adaptive.xml", "anydpi-v26")
        .unwrap();
    table
        .add_file_resource("mipmap", "reseam_round", "res/round.png", "xxhdpi")
        .unwrap();

    // Every density is the same resource: one id, one chunk per configuration.
    assert_eq!(hdpi, xxhdpi);
    assert_eq!(hdpi, anydpi);
    let reparsed = ResourceTable::parse(DexBytes::from_vec(table.serialize().unwrap())).unwrap();
    let package = &reparsed.packages[0];
    let chunk_for = |density: u16, sdk: u16| {
        package.types.iter().find(|res_type| {
            let config = res_type.config();
            config[14..16] == density.to_le_bytes() && config[24..26] == sdk.to_le_bytes()
        })
    };
    let xxhdpi_chunk = chunk_for(480, 0).expect("xxhdpi chunk");
    assert_eq!(xxhdpi_chunk.len(), 3);
    assert!(xxhdpi_chunk.entry(1).is_some() && xxhdpi_chunk.entry(2).is_some());
    assert!(chunk_for(240, 0).expect("hdpi chunk").entry(1).is_some());
    assert!(chunk_for(0xFFFE, 26)
        .expect("anydpi-v26 chunk")
        .entry(1)
        .is_some());
    assert_eq!(package.types.len(), 4);
    let default = package
        .types
        .iter()
        .find(|t| t.is_default_config())
        .unwrap();
    assert!(
        default.entry(1).is_none(),
        "a qualified file stays out of the default configuration"
    );
    assert_eq!(
        reparsed.file_paths("mipmap", "reseam_icon").unwrap(),
        ["res/hdpi.png", "res/xxhdpi.png", "res/adaptive.xml"]
    );
}

#[test]
fn the_package_name_is_renamed_and_survives_a_round_trip() {
    let mut table = table_with_configs(
        "string",
        "app_name",
        &[(vec![0u8; 48], EntryValue::Simple(ResValue::string(0)))],
        &["App"],
    );

    table
        .set_package_name("app.reseam.android.youtube")
        .unwrap();

    let reparsed = ResourceTable::parse(DexBytes::from_vec(table.serialize().unwrap())).unwrap();
    assert_eq!(reparsed.packages[0].name, "app.reseam.android.youtube");
    assert!(reparsed.find_resource_id("string", "app_name").is_some());
    assert!(table
        .set_package_name(&"a".repeat(128))
        .unwrap_err()
        .to_string()
        .contains("longer than a package header holds"));
}

#[test]
fn qualifiers_build_the_config_aapt_would() {
    let config = config_for_qualifiers("night-xxxhdpi-v31", 48).unwrap();
    assert_eq!(&config[..4], &48u32.to_le_bytes());
    assert_eq!(&config[14..16], &640u16.to_le_bytes());
    assert_eq!(&config[24..26], &31u16.to_le_bytes());
    assert_eq!(config[29], 0x20);
    assert_eq!(
        &config_for_qualifiers("420dpi", 48).unwrap()[14..16],
        &420u16.to_le_bytes()
    );
    assert!(config_for_qualifiers("", 48).unwrap()[4..]
        .iter()
        .all(|&b| b == 0));
    assert!(config_for_qualifiers("land", 48)
        .unwrap_err()
        .to_string()
        .contains("unsupported qualifier 'land'"));
}

#[test]
fn style_items_land_in_every_configuration_that_defines_the_style() {
    const WINDOW_BACKGROUND: u32 = 0x0101_0054;
    const TEXT_COLOR: u32 = 0x0101_0098;

    let night = || {
        let mut config = vec![0u8; 48];
        config[13] = 0x20;
        config
    };
    let existing = || EntryValue::Complex {
        parent: 0,
        entries: vec![MapEntry {
            name: TEXT_COLOR,
            value: ResValue::new(ResValue::INT_COLOR_ARGB8, 0xff00_0000),
        }],
    };
    let mut table = table_with_configs(
        "style",
        "AppTheme",
        &[(vec![0u8; 48], existing()), (night(), existing())],
        &[],
    );

    let id = ResourceScope::from(&mut table)
        .set_style_items(
            "AppTheme",
            None,
            &[
                ("android:windowBackground".into(), "#ffffffff".into()),
                ("android:textColor".into(), "#ff102030".into()),
            ],
        )
        .expect("edit the style");

    for res_type in &table.packages[0].types {
        let EntryValue::Complex { entries, .. } = res_type.entry(0).unwrap().value else {
            panic!("expected a bag");
        };
        let names: Vec<u32> = entries.iter().map(|entry| entry.name).collect();
        assert_eq!(names, [WINDOW_BACKGROUND, TEXT_COLOR], "kept in id order");
        assert_eq!(
            entries[1].value.data, 0xff10_2030,
            "the existing item was replaced"
        );
    }

    assert!(ResourceScope::from(&mut table)
        .set_style_items("New.Theme", None, &[])
        .unwrap_err()
        .to_string()
        .contains("pass a parent to create it"));
    let created = ResourceScope::from(&mut table)
        .set_style_items(
            "New.Theme",
            Some(&format!("@0x{id:08x}")),
            &[("android:textColor".into(), "#ff000000".into())],
        )
        .expect("create the style");
    assert_ne!(created, id);
    let EntryValue::Complex { parent, entries } = table.packages[0].types[0]
        .entry((created & 0xFFFF) as usize)
        .unwrap()
        .value
    else {
        panic!("expected a bag");
    };
    assert_eq!(parent, id);
    assert_eq!(entries.len(), 1);

    assert!(ResourceScope::from(&mut table)
        .set_style_items("AppTheme", None, &[("notAnAttribute".into(), "1".into())])
        .unwrap_err()
        .to_string()
        .contains("no attr resource is named notAnAttribute"));
}

#[test]
fn an_array_is_rewritten_with_a_different_element_count() {
    const ARRAY_FIRST_NAME: u32 = 0x0100_0001;

    let mut table = table_with_configs(
        "array",
        "lengths",
        &[(
            vec![0u8; 48],
            EntryValue::Complex {
                parent: 0,
                entries: (0..2)
                    .map(|i| MapEntry {
                        name: ARRAY_FIRST_NAME + i,
                        value: ResValue::int(5 * (i as i32 + 1)),
                    })
                    .collect(),
            },
        )],
        &[],
    );

    assert_eq!(table.array("lengths").unwrap(), ["5", "10"]);
    ResourceScope::from(&mut table)
        .set_array(
            "lengths",
            &["3".into(), "5".into(), "10".into(), "true".into()],
        )
        .expect("rewrite the array");

    assert_eq!(table.array("lengths").unwrap(), ["3", "5", "10", "true"]);
    let EntryValue::Complex { entries, .. } = table.packages[0].types[0].entry(0).unwrap().value
    else {
        panic!("expected a bag");
    };
    let names: Vec<u32> = entries.iter().map(|entry| entry.name).collect();
    assert_eq!(
        names,
        (0..4).map(|i| ARRAY_FIRST_NAME + i).collect::<Vec<_>>()
    );
    assert!(ResourceScope::from(&mut table)
        .set_array("missing", &[])
        .unwrap_err()
        .to_string()
        .contains("the table has no array/missing"));
}

#[test]
fn string_array_values_keep_their_type() {
    let mut table = table_with_configs(
        "array",
        "values",
        &[(
            vec![0u8; 48],
            EntryValue::Complex {
                parent: 0,
                entries: vec![],
            },
        )],
        &[],
    );
    let values = [
        "30",
        "true",
        "@string/missing",
        "?attr/missing",
        "",
        "\\text",
    ]
    .map(String::from);
    table.set_string_array("values", &values).unwrap();
    table
        .set_string_array("values", &table.array("values").unwrap())
        .unwrap();
    let table = ResourceTable::parse(DexBytes::from_vec(table.serialize().unwrap())).unwrap();
    assert_eq!(table.array("values").unwrap(), values);
    assert!(table
        .complex_entries("array", "values")
        .unwrap()
        .iter()
        .all(|entry| entry.value.kind == ResValue::STRING));
}

#[test]
fn attribute_names_with_different_resource_ids_use_different_strings() {
    const RES_AUTO: &str = "http://schemas.android.com/apk/res-auto";
    let mut table = make_test_arsc();
    let app_title = table
        .add_resource("attr", "title", ResValue::new(0, 0))
        .unwrap();
    let mut doc = axml::build_document(
        r#"<Preference xmlns:android="http://schemas.android.com/apk/res/android" android:title="Framework" />"#,
        Some(&mut ResourceScope::from(&mut table)),
    ).unwrap();
    doc.declare_namespace("app", RES_AUTO).unwrap();
    let (_, framework) = doc
        .bind_attribute_name("android:title", Some(&table))
        .unwrap();
    let (_, app) = doc.bind_attribute_name("app:title", Some(&table)).unwrap();
    assert_ne!(framework, app);
    assert_eq!(
        doc.resource_id_for(framework),
        axml::android_attr_res_id("title")
    );
    assert_eq!(doc.resource_id_for(app), Some(app_title));
    assert_eq!(
        doc.bind_attribute(Some(RES_AUTO), "title", Some(&table))
            .unwrap()
            .1,
        app
    );
    let doc = AxmlDocument::parse(&doc.serialize().unwrap()).unwrap();
    assert_eq!(
        doc.resource_id_for(framework),
        axml::android_attr_res_id("title")
    );
    assert_eq!(doc.resource_id_for(app), Some(app_title));
}

#[test]
fn an_attribute_binds_under_whatever_prefix_declares_its_namespace() {
    let mut table = make_test_arsc();
    let attr = table
        .add_resource(
            "attr",
            "layout_constraintRight_toLeftOf",
            ResValue::new(0, 0),
        )
        .expect("app attribute");
    let xml = r#"<FrameLayout
            xmlns:yt="http://schemas.android.com/apk/res-auto"
            xmlns:tools="http://schemas.android.com/tools"
            tools:ignore="ContentDescription"
            yt:layout_constraintRight_toLeftOf="@string/hello" />"#;

    let doc =
        axml::build_document(xml, Some(&mut ResourceScope::from(&mut table))).expect("build axml");
    let element = doc.root().expect("start element");
    assert_eq!(
        doc.attribute(element, attr).map(|a| a.value),
        Some(ResValue::reference(
            table.find_resource_id("string", "hello").unwrap()
        ))
    );
    assert!(
        doc.attribute_named(element, "ignore").is_none(),
        "tools attributes are build-time hints aapt drops"
    );

    let unknown = r#"<FrameLayout xmlns:yt="http://schemas.android.com/apk/res-auto" yt:notAnAttribute="1" />"#;
    assert!(
        axml::build_document(unknown, Some(&mut ResourceScope::from(&mut table)))
            .unwrap_err()
            .to_string()
            .contains("has no attr/notAnAttribute")
    );
    let undeclared = r#"<FrameLayout yt:notAnAttribute="1" />"#;
    assert!(
        axml::build_document(undeclared, Some(&mut ResourceScope::from(&mut table)))
            .unwrap_err()
            .to_string()
            .contains("declares no xmlns:yt")
    );
}

#[test]
fn an_adopted_subtree_is_rebound_in_the_document_that_takes_it() {
    const ANDROID_ID: u32 = 0x0101_00d0;
    const RES_AUTO: &str = "http://schemas.android.com/apk/res-auto";

    let mut table = make_test_arsc();
    let attr = table
        .add_resource(
            "attr",
            "layout_constraintRight_toLeftOf",
            ResValue::new(0, 0),
        )
        .expect("app attribute");
    let source = axml::build_document(
        r#"<FrameLayout
            xmlns:android="http://schemas.android.com/apk/res/android"
            xmlns:yt="http://schemas.android.com/apk/res-auto"
            android:id="@+id/reseam_button"
            yt:layout_constraintRight_toLeftOf="@string/hello">
            <ImageView android:tag="inner" />
        </FrameLayout>"#,
        Some(&mut ResourceScope::from(&mut table)),
    )
    .expect("build the fragment");
    let mut target = axml::build_document(
        r#"<LinearLayout xmlns:android="http://schemas.android.com/apk/res/android" android:tag="root" />"#,
        Some(&mut ResourceScope::from(&mut table)),
    )
    .expect("build the target");

    let start = source.root().unwrap();
    let end = source.find_end_element(start).unwrap();
    let subtree = source.elements[start..=end].to_vec();

    // The target has no res-auto namespace yet, so the adoption says so.
    let refused = target.adopt(&source, &subtree, Some(&table)).unwrap_err();
    assert!(refused.to_string().contains("declares no namespace"));
    assert!(refused.to_string().contains("declareNamespace"));

    target.declare_namespace("app", RES_AUTO).unwrap();
    let adopted = target
        .adopt(&source, &subtree, Some(&table))
        .expect("adopt the fragment");
    let root = target.root().unwrap();
    target.elements.splice(root + 1..root + 1, adopted);

    let container = target.find_element("FrameLayout").expect("the graft");
    let constraint = target
        .attribute(container, attr)
        .expect("the app attribute kept its id");
    assert_eq!(
        constraint.namespace,
        target.namespace_index(RES_AUTO),
        "the source prefix was yt, the target declares app"
    );
    assert_eq!(
        target
            .attribute(container, ANDROID_ID)
            .map(|a| a.value.kind),
        Some(ResValue::REFERENCE)
    );
    let inner = target.find_element("ImageView").expect("the child");
    assert_eq!(
        target
            .attribute_named(inner, "tag")
            .and_then(|attr| target.attribute_string(attr))
            .as_deref(),
        Some("inner"),
        "strings are interned into the target pool"
    );

    let bytes = target.serialize().expect("serialize");
    let reparsed = AxmlDocument::parse(&bytes).expect("reparse");
    assert!(reparsed
        .attribute(reparsed.find_element("FrameLayout").unwrap(), attr)
        .is_some());
}
