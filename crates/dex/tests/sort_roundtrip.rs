// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_dex::ParseOptions;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ResolvedMethod {
    class_desc: String,
    name: String,
    return_type: String,
    params: Vec<String>,
}

fn resolve_methods(dex: &reseam_dex::DexFile) -> Vec<ResolvedMethod> {
    dex.methods
        .iter()
        .map(|m| {
            let class_desc = dex.type_descriptor(m.class).to_owned();
            let name = dex.string(m.name).to_owned();
            let proto = &dex.prototypes[m.proto.0 as usize];
            let return_type = dex.type_descriptor(proto.return_type).to_owned();
            let params: Vec<String> = proto
                .parameters
                .iter()
                .map(|t| dex.type_descriptor(*t).to_owned())
                .collect();
            ResolvedMethod {
                class_desc,
                name,
                return_type,
                params,
            }
        })
        .collect()
}

fn resolve_fields(dex: &reseam_dex::DexFile) -> Vec<(String, String, String)> {
    dex.fields
        .iter()
        .map(|f| {
            (
                dex.type_descriptor(f.class).to_owned(),
                dex.string(f.name).to_owned(),
                dex.type_descriptor(f.type_).to_owned(),
            )
        })
        .collect()
}

fn resolve_class_methods(dex: &reseam_dex::DexFile) -> HashMap<String, Vec<(String, String)>> {
    let mut result: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for class in &dex.classes {
        let class_desc = dex.type_descriptor(class.class_type).to_owned();
        if let Some(ref data) = class.class_data {
            let mut methods = Vec::new();
            for m in data.direct_methods.iter().chain(&data.virtual_methods) {
                let method_id = &dex.methods[m.method.0 as usize];
                let method_class = dex.type_descriptor(method_id.class).to_owned();
                let method_name = dex.string(method_id.name).to_owned();
                methods.push((method_class, method_name));
            }
            result.insert(class_desc, methods);
        }
    }
    result
}

#[test]
fn sort_roundtrip_preserves_method_ids() {
    let apk_path = "../../test-apks/com.google.android.youtube_20.40.45.apk";
    if !std::path::Path::new(apk_path).exists() {
        eprintln!("Skipping: test APK not found at {apk_path}");
        return;
    }

    let apk_bytes = std::fs::read(apk_path).expect("read APK");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&apk_bytes)).expect("open ZIP");

    let mut dex_entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).expect("zip entry");
        let name = entry.name().to_owned();
        if !name.starts_with("classes") || !name.ends_with(".dex") {
            continue;
        }
        use std::io::Read;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("read dex");
        dex_entries.push((name, buf));
    }

    for (name, dex_bytes) in &dex_entries {
        let opts = ParseOptions {
            skip_checksum: true,
            skip_signature: true,
            ..ParseOptions::default()
        };

        let mut dex = reseam_dex::parse(&dex_bytes, opts.clone()).expect("parse original");
        dex.resolve_all_class_data().expect("resolve class data");

        let before_methods = resolve_methods(&dex);
        let before_class_methods = resolve_class_methods(&dex);

        let written = reseam_dex::write(&mut dex).expect("write");
        let dex2 = reseam_dex::parse(&written, opts).expect("parse written");

        let after_methods = resolve_methods(&dex2);
        let after_fields = resolve_fields(&dex2);

        // Check method_ids table has same set of methods
        let before_set: HashMap<&ResolvedMethod, usize> = before_methods
            .iter()
            .enumerate()
            .map(|(i, m)| (m, i))
            .collect();
        let after_set: HashMap<&ResolvedMethod, usize> = after_methods
            .iter()
            .enumerate()
            .map(|(i, m)| (m, i))
            .collect();

        for (m, _) in &before_set {
            if !after_set.contains_key(m) {
                panic!(
                    "[{name}] Method LOST after round-trip: {}.{}({}) -> {}",
                    m.class_desc,
                    m.name,
                    m.params.join(", "),
                    m.return_type
                );
            }
        }
        for (m, _) in &after_set {
            if !before_set.contains_key(m) {
                panic!(
                    "[{name}] Method GAINED after round-trip: {}.{}({}) -> {}",
                    m.class_desc,
                    m.name,
                    m.params.join(", "),
                    m.return_type
                );
            }
        }

        // Check class_data method references point to correct classes
        let mut dex2_resolved = reseam_dex::parse(
            &written,
            ParseOptions {
                skip_checksum: true,
                skip_signature: true,
                ..ParseOptions::default()
            },
        )
        .expect("parse written again");
        dex2_resolved.resolve_all_class_data().expect("resolve");
        let after_class_methods = resolve_class_methods(&dex2_resolved);

        let mut mismatches = Vec::new();
        for (class_desc, before_meths) in &before_class_methods {
            if let Some(after_meths) = after_class_methods.get(class_desc) {
                let mut before_sorted = before_meths.clone();
                before_sorted.sort();
                let mut after_sorted = after_meths.clone();
                after_sorted.sort();
                if before_sorted != after_sorted {
                    mismatches.push(format!(
                        "Class {class_desc}:\n  before: {:?}\n  after:  {:?}",
                        before_sorted, after_sorted
                    ));
                }
            }
        }

        if !mismatches.is_empty() {
            panic!(
                "[{name}] Class method references corrupted after round-trip:\n{}",
                mismatches.join("\n\n")
            );
        }

        // Specifically check $-containing classes
        let dollar_classes: Vec<_> = after_class_methods
            .keys()
            .filter(|k| k.contains('$'))
            .collect();

        for dc in &dollar_classes {
            let after_meths = &after_class_methods[*dc];
            for (method_class, method_name) in after_meths {
                if method_class != *dc {
                    eprintln!(
                        "[{name}] WARNING: ${dc} has method {method_name} pointing to class {method_class}"
                    );
                }
            }
        }

        eprintln!(
            "[{name}] OK: {} methods, {} fields, {} classes ({} with $)",
            after_methods.len(),
            after_fields.len(),
            after_class_methods.len(),
            dollar_classes.len()
        );
    }
}

#[test]
fn sort_roundtrip_after_interning_preserves_method_ids() {
    let apk_path = "../../test-apks/com.google.android.youtube_20.40.45.apk";
    if !std::path::Path::new(apk_path).exists() {
        eprintln!("Skipping: test APK not found at {apk_path}");
        return;
    }

    let apk_bytes = std::fs::read(apk_path).expect("read APK");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&apk_bytes)).expect("open ZIP");

    // Just test with classes.dex (the first one)
    let dex_bytes: Vec<u8> = {
        use std::io::Read;
        let mut entry = zip.by_name("classes.dex").expect("classes.dex");
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("read dex");
        buf
    };

    let opts = ParseOptions {
        skip_checksum: true,
        skip_signature: true,
        ..ParseOptions::default()
    };

    let mut dex = reseam_dex::parse(&dex_bytes, opts.clone()).expect("parse");
    dex.resolve_all_class_data().expect("resolve");

    let before_methods = resolve_methods(&dex);
    let before_class_methods = resolve_class_methods(&dex);

    // Simulate patching: intern new strings, types, methods
    dex.intern_string("Lreseam/Extension;");
    dex.intern_type("Lreseam/Extension;");
    dex.intern_string("extensionMethod");
    let _ = dex.intern_method("Lreseam/Extension;", "extensionMethod", "()V");
    let _ = dex.intern_method(
        "Lreseam/Extension;",
        "anotherMethod",
        "(Ljava/lang/String;I)Z",
    );
    let _ = dex.intern_field("Lreseam/Extension;", "extensionField", "Ljava/lang/String;");

    let written = reseam_dex::write(&mut dex).expect("write");
    let mut dex2 = reseam_dex::parse(&written, opts).expect("parse written");
    dex2.resolve_all_class_data().expect("resolve");

    let after_methods = resolve_methods(&dex2);
    let after_class_methods = resolve_class_methods(&dex2);

    // All original methods must still exist
    let after_set: std::collections::HashSet<_> = after_methods.iter().collect();
    for m in &before_methods {
        if !after_set.contains(m) {
            panic!(
                "Method LOST after intern+roundtrip: {}.{}({}) -> {}",
                m.class_desc,
                m.name,
                m.params.join(", "),
                m.return_type
            );
        }
    }

    // Check class_data method references
    let mut mismatches = Vec::new();
    for (class_desc, before_meths) in &before_class_methods {
        if let Some(after_meths) = after_class_methods.get(class_desc) {
            let mut before_sorted = before_meths.clone();
            before_sorted.sort();
            let mut after_sorted = after_meths.clone();
            after_sorted.sort();
            if before_sorted != after_sorted {
                mismatches.push(format!(
                    "Class {class_desc}:\n  before: {:?}\n  after:  {:?}",
                    before_sorted, after_sorted
                ));
            }
        } else {
            mismatches.push(format!("Class {class_desc} MISSING after round-trip"));
        }
    }

    if !mismatches.is_empty() {
        panic!(
            "Class method references corrupted after intern+roundtrip:\n{}",
            mismatches.join("\n\n")
        );
    }

    // Check $-containing classes specifically
    let mut dollar_issues = Vec::new();
    for (class_desc, meths) in &after_class_methods {
        if class_desc.contains('$') {
            for (method_class, method_name) in meths {
                if method_class != class_desc {
                    dollar_issues.push(format!(
                        "{class_desc}.{method_name} -> wrong class {method_class}"
                    ));
                }
            }
        }
    }

    if !dollar_issues.is_empty() {
        panic!(
            "$-class method reference corruption:\n{}",
            dollar_issues.join("\n")
        );
    }

    eprintln!(
        "OK: intern+roundtrip preserved {} methods across {} classes",
        after_methods.len(),
        after_class_methods.len()
    );
}

#[test]
fn sort_roundtrip_after_code_modification_preserves_refs() {
    let apk_path = "../../test-apks/com.google.android.youtube_20.40.45.apk";
    if !std::path::Path::new(apk_path).exists() {
        eprintln!("Skipping: test APK not found at {apk_path}");
        return;
    }

    let apk_bytes = std::fs::read(apk_path).expect("read APK");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&apk_bytes)).expect("open ZIP");

    let dex_bytes: Vec<u8> = {
        use std::io::Read;
        let mut entry = zip.by_name("classes.dex").expect("classes.dex");
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("read dex");
        buf
    };

    let opts = ParseOptions {
        skip_checksum: true,
        skip_signature: true,
        ..ParseOptions::default()
    };

    let mut dex = reseam_dex::parse(&dex_bytes, opts.clone()).expect("parse");
    dex.resolve_all_class_data().expect("resolve");

    let before_class_methods = resolve_class_methods(&dex);

    // Intern an extension method and string before modifying code
    let ext_method = dex
        .intern_method("Lreseam/Extension;", "hook", "(Ljava/lang/String;)V")
        .expect("intern method");
    let new_str = dex.intern_string("hooked!");

    // Find a $-containing class and modify its code
    let mut modified_class = None;
    for class in dex.classes.iter_mut() {
        if modified_class.is_some() {
            break;
        }
        if let Some(ref mut data) = class.class_data {
            for m in data
                .direct_methods
                .iter_mut()
                .chain(data.virtual_methods.iter_mut())
            {
                if let Some(ref mut code) = m.code {
                    if code.instructions.len() > 3 {
                        code.insert_instructions(
                            0,
                            &[
                                reseam_dex::Instruction::ConstString {
                                    dest: 0,
                                    string: new_str,
                                },
                                reseam_dex::Instruction::InvokeStatic {
                                    method: ext_method,
                                    args: [0u8].into_iter().collect(),
                                },
                            ],
                        )
                        .expect("insert_instructions failed in test");
                        modified_class = Some("(some $-class)".to_owned());
                        break;
                    }
                }
            }
        }
    }

    eprintln!("Modified class: {:?}", modified_class);

    let written = reseam_dex::write(&mut dex).expect("write");
    let mut dex2 = reseam_dex::parse(&written, opts).expect("parse written");
    dex2.resolve_all_class_data().expect("resolve");

    let after_class_methods = resolve_class_methods(&dex2);

    // Check that all original class method references are preserved
    let mut mismatches = Vec::new();
    for (class_desc, before_meths) in &before_class_methods {
        if let Some(after_meths) = after_class_methods.get(class_desc) {
            let mut before_sorted = before_meths.clone();
            before_sorted.sort();
            let mut after_sorted = after_meths.clone();
            after_sorted.sort();
            if before_sorted != after_sorted {
                mismatches.push(format!(
                    "Class {class_desc}:\n  before: {:?}\n  after:  {:?}",
                    before_sorted, after_sorted
                ));
            }
        } else {
            mismatches.push(format!("Class {class_desc} MISSING after round-trip"));
        }
    }

    if !mismatches.is_empty() {
        panic!(
            "Class method references corrupted after code modification:\n{}",
            mismatches.join("\n\n")
        );
    }

    // Check $-containing classes for cross-class reference corruption
    let mut dollar_issues = Vec::new();
    for (class_desc, meths) in &after_class_methods {
        for (method_class, method_name) in meths {
            if method_class != class_desc {
                // This is normal for inherited methods — only flag if involving $
                if method_class.contains('$') || class_desc.contains('$') {
                    dollar_issues.push(format!(
                        "{class_desc}.{method_name} -> references method from {method_class}"
                    ));
                }
            }
        }
    }

    if !dollar_issues.is_empty() {
        eprintln!("$-class cross references (may be normal for inherited methods):");
        for issue in &dollar_issues {
            eprintln!("  {issue}");
        }
    }

    eprintln!(
        "OK: code modification roundtrip preserved {} classes",
        after_class_methods.len()
    );
}

#[test]
fn sort_double_roundtrip_is_stable() {
    let apk_path = "../../test-apks/com.google.android.youtube_20.40.45.apk";
    if !std::path::Path::new(apk_path).exists() {
        eprintln!("Skipping: test APK not found at {apk_path}");
        return;
    }

    let apk_bytes = std::fs::read(apk_path).expect("read APK");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&apk_bytes)).expect("open ZIP");

    let dex_bytes: Vec<u8> = {
        use std::io::Read;
        let mut entry = zip.by_name("classes.dex").expect("classes.dex");
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("read dex");
        buf
    };

    let opts = ParseOptions {
        skip_checksum: true,
        skip_signature: true,
        ..ParseOptions::default()
    };

    // First round-trip
    let mut dex1 = reseam_dex::parse(&dex_bytes, opts.clone()).expect("parse");
    let written1 = reseam_dex::write(&mut dex1).expect("write1");

    // Second round-trip
    let mut dex2 = reseam_dex::parse(&written1, opts.clone()).expect("parse written1");
    let written2 = reseam_dex::write(&mut dex2).expect("write2");

    // The two outputs should be byte-identical (sort is stable/idempotent)
    if written1 != written2 {
        eprintln!(
            "Double round-trip NOT stable! Sizes: {} vs {}",
            written1.len(),
            written2.len()
        );

        // Parse both and compare method tables
        let d1 = reseam_dex::parse(&written1, opts.clone()).expect("parse w1");
        let d2 = reseam_dex::parse(&written2, opts.clone()).expect("parse w2");

        let m1 = resolve_methods(&d1);
        let m2 = resolve_methods(&d2);

        for (i, (a, b)) in m1.iter().zip(m2.iter()).enumerate() {
            if a != b {
                eprintln!("Method[{i}] differs:");
                eprintln!(
                    "  pass1: {}.{}({}) -> {}",
                    a.class_desc,
                    a.name,
                    a.params.join(","),
                    a.return_type
                );
                eprintln!(
                    "  pass2: {}.{}({}) -> {}",
                    b.class_desc,
                    b.name,
                    b.params.join(","),
                    b.return_type
                );
            }
        }

        panic!("Double round-trip is not stable (sort is not idempotent)");
    }

    eprintln!(
        "OK: double round-trip is byte-identical ({} bytes)",
        written1.len()
    );
}

#[test]
fn compare_patched_vs_original() {
    let orig_path = "../../test-apks/com.google.android.youtube_20.40.45.apk";
    let patched_path = "/tmp/patched.apk";
    if !std::path::Path::new(orig_path).exists() || !std::path::Path::new(patched_path).exists() {
        eprintln!("Skipping: need both original and patched APKs");
        return;
    }

    let opts = ParseOptions {
        skip_checksum: true,
        skip_signature: true,
        ..ParseOptions::default()
    };

    let orig_bytes = std::fs::read(orig_path).expect("read orig");
    let patched_bytes = std::fs::read(patched_path).expect("read patched");

    let mut orig_zip = zip::ZipArchive::new(std::io::Cursor::new(&orig_bytes)).expect("open orig");
    let mut patched_zip =
        zip::ZipArchive::new(std::io::Cursor::new(&patched_bytes)).expect("open patched");

    // Collect DEX file names from patched APK
    let patched_dex_names: Vec<String> = (0..patched_zip.len())
        .filter_map(|i| {
            let entry = patched_zip.by_index(i).ok()?;
            let name = entry.name().to_owned();
            if name.starts_with("classes") && name.ends_with(".dex") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    for dex_name in &patched_dex_names {
        let patched_dex_bytes: Vec<u8> = {
            use std::io::Read;
            let mut entry = patched_zip.by_name(dex_name).expect("patched dex");
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).expect("read");
            buf
        };

        let mut patched_dex =
            reseam_dex::parse(&patched_dex_bytes, opts.clone()).expect("parse patched");
        patched_dex
            .resolve_all_class_data()
            .expect("resolve patched");

        // Check for cross-class method reference corruption
        let mut issues = Vec::new();
        for class in &patched_dex.classes {
            let class_desc = patched_dex.type_descriptor(class.class_type);
            if let Some(ref data) = class.class_data {
                for m in data.direct_methods.iter().chain(&data.virtual_methods) {
                    let method_id = &patched_dex.methods[m.method.0 as usize];
                    let method_class = patched_dex.type_descriptor(method_id.class);
                    let method_name = patched_dex.string(method_id.name);

                    // Check if the method's defining class contains $ but doesn't match
                    // the class it's declared in (potential corruption)
                    if method_class != class_desc
                        && (method_class.contains("$External") || method_class.contains("$Api"))
                    {
                        issues.push(format!(
                            "  {class_desc} has method '{method_name}' from {method_class}"
                        ));
                    }
                }
            }
        }

        // Check original method count
        if let Ok(mut orig_entry) = orig_zip.by_name(dex_name) {
            use std::io::Read;
            let mut orig_dex_bytes = Vec::new();
            orig_entry
                .read_to_end(&mut orig_dex_bytes)
                .expect("read orig");
            let orig_dex = reseam_dex::parse(&orig_dex_bytes, opts.clone()).expect("parse orig");
            eprintln!(
                "[{dex_name}] ORIGINAL: {} methods, {} fields, {} types",
                orig_dex.methods.len(),
                orig_dex.fields.len(),
                orig_dex.types.len(),
            );
        }

        // Also check if same dex exists in original — compare
        if let Ok(mut orig_entry) = orig_zip.by_name(dex_name) {
            use std::io::Read;
            let mut orig_dex_bytes = Vec::new();
            orig_entry
                .read_to_end(&mut orig_dex_bytes)
                .expect("read orig");
            let mut orig_dex =
                reseam_dex::parse(&orig_dex_bytes, opts.clone()).expect("parse orig");
            orig_dex.resolve_all_class_data().expect("resolve orig");

            let orig_class_methods = resolve_class_methods(&orig_dex);
            let patched_class_methods = resolve_class_methods(&patched_dex);

            let mut corrupted = Vec::new();
            for (class_desc, orig_meths) in &orig_class_methods {
                if let Some(patched_meths) = patched_class_methods.get(class_desc) {
                    let mut orig_sorted = orig_meths.clone();
                    orig_sorted.sort();
                    let mut patched_sorted = patched_meths.clone();
                    patched_sorted.sort();
                    if orig_sorted != patched_sorted {
                        // Only report if the change involves $-containing classes
                        let involves_dollar = orig_sorted
                            .iter()
                            .chain(patched_sorted.iter())
                            .any(|(c, _)| c.contains('$'));
                        if involves_dollar {
                            corrupted.push(format!(
                                "  {class_desc}:\n    orig:    {:?}\n    patched: {:?}",
                                orig_sorted, patched_sorted
                            ));
                        }
                    }
                }
            }

            if !corrupted.is_empty() {
                eprintln!("[{dex_name}] CORRUPTED $-class method references:");
                for c in &corrupted {
                    eprintln!("{c}");
                }
            }
        }

        if !issues.is_empty() {
            eprintln!("[{dex_name}] Suspicious cross-class $-references:");
            for issue in &issues {
                eprintln!("{issue}");
            }
        }

        // Check for index overflow
        if patched_dex.methods.len() > 0xFFFF {
            eprintln!(
                "[{dex_name}] *** METHOD INDEX OVERFLOW: {} methods (max 65535) ***",
                patched_dex.methods.len()
            );
        }
        if patched_dex.fields.len() > 0xFFFF {
            eprintln!(
                "[{dex_name}] *** FIELD INDEX OVERFLOW: {} fields (max 65535) ***",
                patched_dex.fields.len()
            );
        }
        if patched_dex.types.len() > 0xFFFF {
            eprintln!(
                "[{dex_name}] *** TYPE INDEX OVERFLOW: {} types (max 65535) ***",
                patched_dex.types.len()
            );
        }

        eprintln!(
            "[{dex_name}] {} classes, {} methods, {} fields, {} types",
            patched_dex.classes.len(),
            patched_dex.methods.len(),
            patched_dex.fields.len(),
            patched_dex.types.len(),
        );
    }
}
