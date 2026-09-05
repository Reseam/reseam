// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_apk::axml::{self, AxmlAttribute, AxmlDocument, AxmlEvent};
use reseam_apk::resources::{
    EntryValue, MapEntry, ResEntry, ResPackage, ResType, ResourceTable, TypeSpec,
};
use reseam_apk::{ResValue, StringPool};
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
            android:padding="16dp"
            android:alpha="0.5" />
    "#;

    let doc = axml::build_document(xml, Some(&mut table)).expect("build axml");
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
    assert_eq!(attr("theme"), ResValue::attribute(local_attr_id));
    assert_eq!(attr("textColor"), ResValue::attribute(android_attr_id));
    assert_eq!(attr("padding").kind, ResValue::DIMENSION);
    assert_eq!(attr("alpha").kind, ResValue::FLOAT);
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
