use std::io::Read;
use stitch_apk::dex;
use stitch_dex::{InstructionPattern, OpcodeMatcher, ParseOptions};

const YOUTUBE_APK: &str = "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";
const INSTAGRAM_APK: &str = "../../test-apks/com.instagram.android_419.0.0.49.71-382508603_minAPI28(arm64-v8a)(360,400,420,480dpi)_apkmirror.com.apk";

fn available_apks() -> Vec<&'static str> {
    [YOUTUBE_APK, INSTAGRAM_APK]
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect()
}

fn extract_dex_files_from_apk(apk_path: &str) -> Vec<(String, Vec<u8>)> {
    let file = std::fs::File::open(apk_path).expect("Failed to open APK");
    let mut archive = zip::ZipArchive::new(file).expect("Failed to read ZIP");
    let mut dex_files = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().to_string();
        if name.ends_with(".dex") {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).unwrap();
            dex_files.push((name, buf));
        }
    }
    dex_files
}

#[test]
fn test_parse_all_apk_dex_files() {
    let apks = available_apks();
    if apks.is_empty() {
        eprintln!("Skipping: no APK files found");
        return;
    }

    for apk in &apks {
        eprintln!("\n=== {} ===", apk);
        let dex_files = extract_dex_files_from_apk(apk);
        assert!(!dex_files.is_empty(), "No DEX files found in {}", apk);

        for (name, buf) in &dex_files {
            let mut dex = stitch_dex::parse(buf, ParseOptions::default())
                .unwrap_or_else(|e| panic!("Failed to parse {} in {}: {}", name, apk, e));
            eprintln!(
                "  {}: {} strings, {} types, {} methods, {} classes ({} bytes)",
                name,
                dex.strings.len(),
                dex.types.len(),
                dex.methods.len(),
                dex.classes.len(),
                buf.len(),
            );
        }
    }
}

#[test]
fn test_round_trip_all_apks() {
    let apks = available_apks();
    if apks.is_empty() {
        eprintln!("Skipping: no APK files found");
        return;
    }

    for apk in &apks {
        eprintln!("\n=== Round-trip: {} ===", apk);
        let dex_files = extract_dex_files_from_apk(apk);

        for (name, buf) in &dex_files {
            let mut dex = stitch_dex::parse(buf, ParseOptions::default())
                .unwrap_or_else(|e| panic!("Failed to parse {} in {}: {}", name, apk, e));

            let output = stitch_dex::write(&mut dex)
                .unwrap_or_else(|e| panic!("Failed to write {} from {}: {}", name, apk, e));

            let dex2 = stitch_dex::parse(&output, ParseOptions::default())
                .unwrap_or_else(|e| panic!("Failed to re-parse {} from {}: {}", name, apk, e));

            assert_eq!(
                dex.strings.len(),
                dex2.strings.len(),
                "{} ({}): string count mismatch",
                name,
                apk
            );
            assert_eq!(
                dex.types.len(),
                dex2.types.len(),
                "{} ({}): type count mismatch",
                name,
                apk
            );
            assert_eq!(
                dex.methods.len(),
                dex2.methods.len(),
                "{} ({}): method count mismatch",
                name,
                apk
            );
            assert_eq!(
                dex.classes.len(),
                dex2.classes.len(),
                "{} ({}): class count mismatch",
                name,
                apk
            );

            eprintln!("  {}: {} -> {} bytes OK", name, buf.len(), output.len());
        }
    }
}

#[test]
fn test_multi_dex_container() {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let buffers: Vec<&[u8]> = dex_files.iter().map(|(_, b)| b.as_slice()).collect();

    let mut container = stitch_dex::MultiDexContainer::parse(&buffers, ParseOptions::default())
        .expect("Failed to parse multi-dex");

    assert_eq!(container.len(), dex_files.len());

    let found = container.find_class("Landroid/support/v4/app/Fragment;");
    if let Some((dex_idx, _class)) = found {
        eprintln!("Found Fragment in dex {}", dex_idx);
    }

    let outputs = container.write_all().expect("Failed to write multi-dex");
    assert_eq!(outputs.len(), dex_files.len());

    for (i, output) in outputs.iter().enumerate() {
        let reparsed = stitch_dex::parse(output, ParseOptions::default())
            .unwrap_or_else(|_| panic!("Failed to re-parse multi-dex {}", i));
        let original = container.dex(i).unwrap();
        assert_eq!(
            original.classes.len(),
            reparsed.classes.len(),
            "dex {}: class count mismatch",
            i
        );
        eprintln!("multi-dex {}: {} bytes, round-trip OK", i, output.len());
    }
}

#[test]
fn test_from_apk() {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let apk_bytes = std::fs::read(YOUTUBE_APK).expect("Failed to read APK");
    let container =
        dex::from_apk(&apk_bytes, ParseOptions::default()).expect("Failed to parse from APK");

    assert!(!container.is_empty(), "Should have found DEX files in APK");
    eprintln!("from_apk: found {} DEX files", container.len());
}

#[test]
fn test_intern_method_and_field() {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let (_, buf) = &dex_files[0];

    let mut dex = stitch_dex::parse(buf, ParseOptions::default()).expect("Failed to parse");

    let original_method_count = dex.methods.len();
    let original_field_count = dex.fields.len();

    let method_idx = dex
        .intern_method("Lcom/example/Test;", "doStuff", "(II)V")
        .expect("valid");
    assert_eq!(dex.methods.len(), original_method_count + 1);
    let method_idx2 = dex
        .intern_method("Lcom/example/Test;", "doStuff", "(II)V")
        .expect("valid");
    assert_eq!(method_idx, method_idx2);
    assert_eq!(dex.methods.len(), original_method_count + 1);

    let field_idx = dex
        .intern_field("Lcom/example/Test;", "count", "I")
        .expect("valid");
    assert_eq!(dex.fields.len(), original_field_count + 1);
    let field_idx2 = dex
        .intern_field("Lcom/example/Test;", "count", "I")
        .expect("valid");
    assert_eq!(field_idx, field_idx2);
    assert_eq!(dex.fields.len(), original_field_count + 1);

    let output = stitch_dex::write(&mut dex).expect("Failed to write");
    let dex2 = stitch_dex::parse(&output, ParseOptions::default()).expect("Failed to re-parse");
    assert_eq!(dex2.methods.len(), original_method_count + 1);
    assert_eq!(dex2.fields.len(), original_field_count + 1);
}

#[test]
fn test_fingerprint_search() {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let (_, buf) = &dex_files[0];
    let dex = stitch_dex::parse(buf, ParseOptions::default()).expect("Failed to parse");

    let init_methods =
        dex.find_methods_by(|method_id, _class, _em| dex.string(method_id.name) == "<init>");
    assert!(!init_methods.is_empty());

    let pattern = [
        InstructionPattern::Opcode(OpcodeMatcher::Const),
        InstructionPattern::Opcode(OpcodeMatcher::Return),
    ];
    let matches = dex.find_methods_with_opcodes(&pattern);
    eprintln!("Found {} methods matching [Const, Return]", matches.len());

    let pattern2 = [
        InstructionPattern::Any,
        InstructionPattern::Opcode(OpcodeMatcher::ReturnVoid),
    ];
    let matches2 = dex.find_methods_with_opcodes(&pattern2);
    assert!(!matches2.is_empty());
}

#[test]
fn test_mutation_write_reparse() {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let (_, buf) = &dex_files[0];
    let mut dex = stitch_dex::parse(buf, ParseOptions::default()).expect("Failed to parse");

    let mut patched_count = 0;
    for class in &mut dex.classes {
        if let Some(ref mut data) = class.class_data {
            for m in data
                .direct_methods
                .iter_mut()
                .chain(data.virtual_methods.iter_mut())
            {
                if let Some(ref mut code) = m.code {
                    if code.instructions.len() > 5 && patched_count < 3 {
                        code.return_early();
                        patched_count += 1;
                    }
                }
            }
        }
        if patched_count >= 3 {
            break;
        }
    }
    assert_eq!(patched_count, 3);

    let output = stitch_dex::write(&mut dex).expect("Failed to write");
    let dex2 = stitch_dex::parse(&output, ParseOptions::default()).expect("Failed to re-parse");
    assert_eq!(dex.classes.len(), dex2.classes.len());
    assert_eq!(dex.strings.len(), dex2.strings.len());
}

#[test]
fn test_raw_buffer_retained() {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let (_, buf) = &dex_files[0];
    let dex = stitch_dex::parse(buf, ParseOptions::default()).expect("Failed to parse");
    assert!(dex.raw.is_some());
    assert_eq!(dex.raw_buffer().unwrap().len(), buf.len());
}

#[test]
fn test_lazy_parsing() {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let (_, buf) = &dex_files[0];

    let mut dex = stitch_dex::parse(
        buf,
        ParseOptions {
            lazy: true,
            ..ParseOptions::default()
        },
    )
    .expect("Failed to parse lazily");

    assert!(dex.is_lazy());
    assert!(dex.raw.is_some());
    assert!(!dex.strings.is_empty());
    assert!(!dex.classes.is_empty());

    let any_has_data = dex.classes.iter().any(|c| c.class_data.is_some());
    assert!(!any_has_data);

    dex.resolve_class_data(0)
        .expect("Failed to resolve class 0");
    dex.resolve_all_class_data().expect("Failed to resolve all");
    assert!(!dex.is_lazy());

    let classes_with_data = dex
        .classes
        .iter()
        .filter(|c| c.class_data.is_some())
        .count();
    assert!(classes_with_data > 0);

    let output = stitch_dex::write(&mut dex).expect("Failed to write");
    let dex2 = stitch_dex::parse(&output, ParseOptions::default()).expect("Failed to re-parse");
    assert_eq!(dex.classes.len(), dex2.classes.len());
}
