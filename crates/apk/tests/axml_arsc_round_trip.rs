use stitch_apk::axml::{AxmlAttribute, AxmlDocument, AxmlEvent, StringPool, TypedValue};
use stitch_apk::resources::{
    MapEntry, ResConfig, ResEntry, ResPackage, ResType, ResValue, ResourceTable, TypeSpec,
};

fn make_test_axml() -> AxmlDocument {
    let pool = StringPool {
        strings: vec![
            "http://schemas.android.com/apk/res/android".to_string(),
            "android".to_string(),
            "manifest".to_string(),
            "package".to_string(),
            "versionCode".to_string(),
            "versionName".to_string(),
            "com.example.test".to_string(),
            "1.0.0".to_string(),
        ],
        is_utf8: true,
    };

    let resource_ids = vec![0, 0, 0, 0, 0x0101_021b, 0x0101_021c];

    let elements = vec![
        AxmlEvent::StartNamespace {
            prefix: Some(1),
            uri: 0,
        },
        AxmlEvent::StartElement {
            namespace: None,
            name: 2,
            attributes: vec![
                AxmlAttribute {
                    namespace: None,
                    name: 3,
                    raw_value: Some(6),
                    typed_value: TypedValue::String(6),
                },
                AxmlAttribute {
                    namespace: Some(0),
                    name: 4,
                    raw_value: None,
                    typed_value: TypedValue::Int(1),
                },
                AxmlAttribute {
                    namespace: Some(0),
                    name: 5,
                    raw_value: Some(7),
                    typed_value: TypedValue::String(7),
                },
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
        string_pool: pool,
        resource_ids,
        elements,
    }
}

#[test]
fn test_axml_round_trip_synthetic() {
    let doc = make_test_axml();

    let bytes = doc.serialize().expect("serialize failed");
    let reparsed = AxmlDocument::parse(&bytes).expect("reparse failed");

    assert_eq!(
        doc.string_pool.strings.len(),
        reparsed.string_pool.strings.len()
    );
    for (i, (a, b)) in doc
        .string_pool
        .strings
        .iter()
        .zip(&reparsed.string_pool.strings)
        .enumerate()
    {
        assert_eq!(a, b, "string {i} mismatch");
    }

    assert_eq!(doc.resource_ids, reparsed.resource_ids);
    assert_eq!(doc.elements.len(), reparsed.elements.len());

    assert_eq!(reparsed.package_name(), Some("com.example.test"));
    assert_eq!(reparsed.version_code(), Some(1));
    assert_eq!(reparsed.version_name(), Some("1.0.0"));
}

#[test]
fn test_axml_round_trip_utf16() {
    let mut doc = make_test_axml();
    doc.string_pool.is_utf8 = false;

    let bytes = doc.serialize().expect("serialize failed");
    let reparsed = AxmlDocument::parse(&bytes).expect("reparse failed");

    assert_eq!(
        doc.string_pool.strings.len(),
        reparsed.string_pool.strings.len()
    );
    for (i, (a, b)) in doc
        .string_pool
        .strings
        .iter()
        .zip(&reparsed.string_pool.strings)
        .enumerate()
    {
        assert_eq!(a, b, "string {i} mismatch");
    }

    assert_eq!(reparsed.package_name(), Some("com.example.test"));
}

#[test]
fn test_axml_round_trip_mutated() {
    let mut doc = make_test_axml();
    doc.set_version_code(42);
    doc.set_version_name("2.0.0");

    let bytes = doc.serialize().expect("serialize failed");
    let reparsed = AxmlDocument::parse(&bytes).expect("reparse failed");

    assert_eq!(reparsed.version_code(), Some(42));
    assert_eq!(reparsed.version_name(), Some("2.0.0"));
    assert_eq!(reparsed.package_name(), Some("com.example.test"));
}

#[test]
fn test_axml_round_trip_add_permission() {
    let mut doc = make_test_axml();
    let original_element_count = doc.elements.len();
    doc.add_permission("android.permission.INTERNET");

    let bytes = doc.serialize().expect("serialize failed");
    let reparsed = AxmlDocument::parse(&bytes).expect("reparse failed");

    assert_eq!(reparsed.elements.len(), original_element_count + 2);
    assert_eq!(reparsed.package_name(), Some("com.example.test"));
}

fn make_test_arsc() -> ResourceTable {
    ResourceTable {
        global_strings: vec![
            "Hello".to_string(),
            "World".to_string(),
            "app_name".to_string(),
        ],
        packages: vec![ResPackage {
            id: 0x7F,
            name: "com.example.test".to_string(),
            type_strings: vec!["string".to_string()],
            key_strings: vec!["hello".to_string(), "world".to_string()],
            type_specs: vec![TypeSpec {
                id: 1,
                flags: vec![0, 0],
            }],
            types: vec![ResType {
                id: 1,
                config: ResConfig { data: vec![0; 48] },
                entries: vec![
                    Some(ResEntry {
                        flags: 0,
                        key: 0,
                        value: ResValue::Simple {
                            data_type: 0x03,
                            data: 0,
                        },
                    }),
                    Some(ResEntry {
                        flags: 0,
                        key: 1,
                        value: ResValue::Simple {
                            data_type: 0x03,
                            data: 1,
                        },
                    }),
                ],
            }],
        }],
    }
}

#[test]
fn test_find_resource_id_across_packages() {
    let mut table = make_test_arsc();
    table.packages.insert(
        0,
        ResPackage {
            id: 0x7E,
            name: "empty.pkg".to_string(),
            type_strings: vec![],
            key_strings: vec![],
            type_specs: vec![],
            types: vec![],
        },
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
        .add_resource("attr", "titleText", 0, 0)
        .expect("attr id");
    let android_attr_id =
        stitch_apk::axml::compiler::android_attr_res_id("textColor").expect("android attr id");
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

    let doc = stitch_apk::axml::compiler::build_axml_document_with_resources(xml, Some(&mut table))
        .expect("build axml");
    let element = doc
        .elements
        .iter()
        .find_map(|event| match event {
            AxmlEvent::StartElement { attributes, .. } => Some(attributes),
            _ => None,
        })
        .expect("start element");
    let attr = |name: &str| {
        element
            .iter()
            .find(|attr| doc.string(attr.name) == Some(name))
            .expect("attribute present")
    };

    assert!(matches!(
        attr("text").typed_value,
        TypedValue::Reference(id) if id == string_id
    ));
    assert!(matches!(
        attr("id").typed_value,
        TypedValue::Reference(id) if id == table.find_resource_id("id", "title").unwrap()
    ));
    assert!(matches!(
        attr("theme").typed_value,
        TypedValue::Other { data_type: 0x02, data } if data == local_attr_id
    ));
    assert!(matches!(
        attr("textColor").typed_value,
        TypedValue::Other { data_type: 0x02, data } if data == android_attr_id
    ));
    assert!(matches!(
        attr("padding").typed_value,
        TypedValue::Other {
            data_type: 0x05,
            ..
        }
    ));
    assert!(matches!(
        attr("alpha").typed_value,
        TypedValue::Other {
            data_type: 0x04,
            ..
        }
    ));
}

#[test]
fn test_arsc_round_trip_synthetic() {
    let table = make_test_arsc();

    let bytes = table.serialize().expect("serialize failed");
    let reparsed = ResourceTable::parse(&bytes).expect("reparse failed");

    assert_eq!(table.global_strings, reparsed.global_strings);
    assert_eq!(table.packages.len(), reparsed.packages.len());

    let pkg = &reparsed.packages[0];
    assert_eq!(pkg.id, 0x7F);
    assert_eq!(pkg.name, "com.example.test");
    assert_eq!(pkg.type_strings, vec!["string"]);
    assert_eq!(pkg.key_strings, vec!["hello", "world"]);
    assert_eq!(pkg.type_specs.len(), 1);
    assert_eq!(pkg.types.len(), 1);

    let t = &pkg.types[0];
    assert_eq!(t.id, 1);
    assert_eq!(t.entries.len(), 2);
    assert!(t.entries[0].is_some());
    assert!(t.entries[1].is_some());

    let e0 = t.entries[0].as_ref().unwrap();
    assert_eq!(e0.key, 0);
    match &e0.value {
        ResValue::Simple { data_type, data } => {
            assert_eq!(*data_type, 0x03);
            assert_eq!(*data, 0);
        }
        _ => panic!("expected simple value"),
    }
}

#[test]
fn test_arsc_round_trip_complex_entries() {
    let table = ResourceTable {
        global_strings: vec!["test".to_string()],
        packages: vec![ResPackage {
            id: 0x7F,
            name: "com.example".to_string(),
            type_strings: vec!["style".to_string()],
            key_strings: vec!["AppTheme".to_string()],
            type_specs: vec![TypeSpec {
                id: 1,
                flags: vec![0],
            }],
            types: vec![ResType {
                id: 1,
                config: ResConfig { data: vec![0; 48] },
                entries: vec![Some(ResEntry {
                    flags: 0x0001,
                    key: 0,
                    value: ResValue::Complex {
                        parent: 0x01030005,
                        entries: vec![
                            MapEntry {
                                name: 0x010100D4,
                                data_type: 0x01,
                                data: 0x7F020001,
                            },
                            MapEntry {
                                name: 0x010100D5,
                                data_type: 0x01,
                                data: 0x7F020002,
                            },
                        ],
                    },
                })],
            }],
        }],
    };

    let bytes = table.serialize().expect("serialize failed");
    let reparsed = ResourceTable::parse(&bytes).expect("reparse failed");

    let entry = reparsed.packages[0].types[0].entries[0].as_ref().unwrap();
    match &entry.value {
        ResValue::Complex { parent, entries } => {
            assert_eq!(*parent, 0x01030005);
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].name, 0x010100D4);
            assert_eq!(entries[0].data, 0x7F020001);
            assert_eq!(entries[1].name, 0x010100D5);
            assert_eq!(entries[1].data, 0x7F020002);
        }
        _ => panic!("expected complex value"),
    }
}

#[test]
fn test_arsc_round_trip_mutated() {
    let mut table = make_test_arsc();
    table.set_string(0, "Modified".to_string());

    let refs = table.find_entries_by_string(1);
    assert_eq!(refs.len(), 1);
    table.replace_entry_string(refs[0].res_id, 0);

    let bytes = table.serialize().expect("serialize failed");
    let reparsed = ResourceTable::parse(&bytes).expect("reparse failed");

    assert_eq!(reparsed.global_strings[0], "Modified");

    let entry = reparsed.packages[0].types[0].entries[1].as_ref().unwrap();
    match &entry.value {
        ResValue::Simple { data, .. } => assert_eq!(*data, 0),
        _ => panic!("expected simple value"),
    }
}

#[test]
fn test_arsc_round_trip_with_none_entries() {
    let mut table = make_test_arsc();
    table.packages[0].types[0].entries.push(None);
    table.packages[0].types[0].entries.push(None);
    table.packages[0].type_specs[0].flags.push(0);
    table.packages[0].type_specs[0].flags.push(0);

    let bytes = table.serialize().expect("serialize failed");
    let reparsed = ResourceTable::parse(&bytes).expect("reparse failed");

    let t = &reparsed.packages[0].types[0];
    assert_eq!(t.entries.len(), 4);
    assert!(t.entries[0].is_some());
    assert!(t.entries[1].is_some());
    assert!(t.entries[2].is_none());
    assert!(t.entries[3].is_none());
}

const YOUTUBE_APK: &str = "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";
const INSTAGRAM_APK: &str = "../../test-apks/com.instagram.android_419.0.0.49.71-382508603_minAPI28(arm64-v8a)(360,400,420,480dpi)_apkmirror.com.apk";

fn available_apks() -> Vec<&'static str> {
    [YOUTUBE_APK, INSTAGRAM_APK]
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect()
}

#[test]
fn test_axml_round_trip_real_apks() {
    let apks = available_apks();
    if apks.is_empty() {
        eprintln!("Skipping: no APK files found");
        return;
    }

    for apk_path in &apks {
        eprintln!("\n=== AXML round-trip: {apk_path} ===");
        let file = std::fs::File::open(apk_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let manifest_bytes = {
            let mut entry = archive.by_name("AndroidManifest.xml").unwrap();
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf).unwrap();
            buf
        };

        let doc = AxmlDocument::parse(&manifest_bytes).expect("parse failed");
        let serialized = doc.serialize().expect("serialize failed");
        let reparsed = AxmlDocument::parse(&serialized).expect("reparse failed");

        assert_eq!(
            doc.string_pool.strings.len(),
            reparsed.string_pool.strings.len()
        );
        assert_eq!(doc.resource_ids, reparsed.resource_ids);
        assert_eq!(doc.elements.len(), reparsed.elements.len());
        assert_eq!(doc.package_name(), reparsed.package_name());
        assert_eq!(doc.version_code(), reparsed.version_code());
        assert_eq!(doc.version_name(), reparsed.version_name());

        eprintln!(
            "  package={:?} version={:?} strings={} elements={} OK",
            reparsed.package_name(),
            reparsed.version_name(),
            reparsed.string_pool.strings.len(),
            reparsed.elements.len(),
        );
    }
}

#[test]
fn test_arsc_round_trip_real_apks() {
    let apks = available_apks();
    if apks.is_empty() {
        eprintln!("Skipping: no APK files found");
        return;
    }

    for apk_path in &apks {
        eprintln!("\n=== ARSC round-trip: {apk_path} ===");
        let file = std::fs::File::open(apk_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let arsc_bytes = {
            let mut entry = archive.by_name("resources.arsc").unwrap();
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf).unwrap();
            buf
        };

        let table = ResourceTable::parse(&arsc_bytes).expect("parse failed");
        let serialized = table.serialize().expect("serialize failed");
        let reparsed = ResourceTable::parse(&serialized).expect("reparse failed");

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

        eprintln!(
            "  strings={} packages={} OK",
            reparsed.global_strings.len(),
            reparsed.packages.len(),
        );
    }
}

#[test]
fn test_arsc_mutation_preserves_real_type_header_sizes() {
    let apks = available_apks();
    if apks.is_empty() {
        eprintln!("Skipping: no APK files found");
        return;
    }

    for apk_path in &apks {
        eprintln!("\n=== ARSC mutation header preservation: {apk_path} ===");
        let file = std::fs::File::open(apk_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let arsc_bytes = {
            let mut entry = archive.by_name("resources.arsc").unwrap();
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf).unwrap();
            buf
        };

        let original_headers = collect_type_header_sizes(&arsc_bytes).expect("header scan failed");
        let mut table = ResourceTable::parse(&arsc_bytes).expect("parse failed");
        table.add_global_string("stitch mutation sentinel");
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
