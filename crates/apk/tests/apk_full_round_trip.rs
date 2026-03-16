use stitch_apk::ApkFile;

const YOUTUBE_APK: &str =
    "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";
const INSTAGRAM_APK: &str = "../../test-apks/com.instagram.android_419.0.0.49.71-382508603_minAPI28(arm64-v8a)(360,400,420,480dpi)_apkmirror.com.apk";

fn available_apks() -> Vec<&'static str> {
    [YOUTUBE_APK, INSTAGRAM_APK]
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect()
}

#[test]
fn test_apk_full_round_trip() {
    let apks = available_apks();
    if apks.is_empty() {
        eprintln!("Skipping: no APK files found");
        return;
    }

    for apk_path in &apks {
        eprintln!("\n=== full round-trip: {apk_path} ===");

        // --- Phase 1: write and verify ZIP structure ---
        let apk = ApkFile::open(apk_path).expect("open failed");
        let original_dex_count = apk.dex().len();
        let original_package = apk.package_name().map(|s| s.to_owned());
        let original_resource_strings = apk
            .resources()
            .map(|r| r.global_strings.len())
            .unwrap_or(0);

        let tmp = tempfile::tempdir().expect("tempdir failed");
        apk.write_to(tmp.path()).expect("write_to failed");
        drop(apk);

        let output_path = find_apk_in_dir(tmp.path());
        {
            let file = std::fs::File::open(&output_path).expect("open output failed");
            let mut archive = zip::ZipArchive::new(file).expect("not a valid ZIP");

            assert!(
                archive.index_for_name("AndroidManifest.xml").is_some(),
                "missing AndroidManifest.xml"
            );

            let dex_count = (0..archive.len())
                .filter(|i| {
                    archive
                        .by_index_raw(*i)
                        .map(|e| e.name().ends_with(".dex"))
                        .unwrap_or(false)
                })
                .count();
            assert_eq!(dex_count, original_dex_count, "DEX count mismatch");
        }

        // --- Phase 2: reopen and verify resources preserved ---
        let apk2 = ApkFile::open(&output_path).expect("reopen failed");
        assert_eq!(apk2.package_name().map(|s| s.to_owned()), original_package);
        assert_eq!(apk2.dex().len(), original_dex_count);
        assert_eq!(
            apk2.resources()
                .map(|r| r.global_strings.len())
                .unwrap_or(0),
            original_resource_strings
        );
        drop(apk2);

        eprintln!("  write + verify OK (dex={original_dex_count} strings={original_resource_strings})");

        // --- Phase 3: mutate DEX, write, reparse ---
        let mut apk3 = ApkFile::open(apk_path).expect("open for dex mutation failed");
        let mut patched = 0;
        for i in 0..apk3.dex().len() {
            let dex = apk3.dex_mut().dex_mut(i).expect("dex access failed");
            for class in &mut dex.classes {
                if let Some(ref mut data) = class.class_data {
                    for m in data
                        .direct_methods
                        .iter_mut()
                        .chain(data.virtual_methods.iter_mut())
                    {
                        if let Some(ref mut code) = m.code {
                            if code.instructions.len() > 5 && patched < 3 {
                                code.return_early();
                                patched += 1;
                            }
                        }
                    }
                }
                if patched >= 3 {
                    break;
                }
            }
            if patched >= 3 {
                break;
            }
        }
        assert_eq!(patched, 3);

        let tmp3 = tempfile::tempdir().expect("tempdir failed");
        apk3.write_to(tmp3.path()).expect("write_to failed");
        let out3 = find_apk_in_dir(tmp3.path());
        let apk3r = ApkFile::open(&out3).expect("reopen dex-mutated failed");
        assert_eq!(apk3r.dex().len(), original_dex_count);
        assert_eq!(apk3r.package_name().map(|s| s.to_owned()), original_package);
        drop(apk3);
        drop(apk3r);

        eprintln!("  dex mutation round-trip OK (patched {patched} methods)");

        // --- Phase 4: mutate manifest, write, reparse ---
        let mut apk4 = ApkFile::open(apk_path).expect("open for manifest mutation failed");
        apk4.manifest_mut().set_version_code(99999);
        apk4.manifest_mut().set_version_name("99.0.0-stitch");

        let tmp4 = tempfile::tempdir().expect("tempdir failed");
        apk4.write_to(tmp4.path()).expect("write_to failed");
        let out4 = find_apk_in_dir(tmp4.path());
        let apk4r = ApkFile::open(&out4).expect("reopen manifest-mutated failed");
        assert_eq!(apk4r.version_code(), Some(99999));
        assert_eq!(apk4r.version_name(), Some("99.0.0-stitch"));
        drop(apk4);
        drop(apk4r);

        eprintln!("  manifest mutation round-trip OK");

        // --- Phase 5: write + sign ---
        let apk5 = ApkFile::open(apk_path).expect("open for sign test failed");
        let tmp5 = tempfile::tempdir().expect("tempdir failed");
        apk5.write_to(tmp5.path()).expect("write_to failed");
        drop(apk5);

        let out5 = find_apk_in_dir(tmp5.path());
        let unsigned_bytes = std::fs::read(&out5).expect("read failed");

        let key = stitch_sign::SigningKey::generate().expect("keygen failed");
        let signed_bytes =
            stitch_sign::v2::sign(&unsigned_bytes, &key).expect("signing failed");

        assert!(signed_bytes.len() > unsigned_bytes.len());

        let cursor = std::io::Cursor::new(&signed_bytes);
        let mut archive = zip::ZipArchive::new(cursor).expect("signed APK is not valid ZIP");
        assert!(archive.index_for_name("AndroidManifest.xml").is_some());

        let dex_count = (0..archive.len())
            .filter(|i| {
                archive
                    .by_index_raw(*i)
                    .map(|e| e.name().ends_with(".dex"))
                    .unwrap_or(false)
            })
            .count();
        assert!(dex_count > 0, "no DEX in signed APK");

        eprintln!(
            "  sign OK (unsigned={} signed={} dex={dex_count})",
            unsigned_bytes.len(),
            signed_bytes.len(),
        );

        // --- Phase 6: full pipeline (mutate + write + sign + reparse) ---
        let mut apk6 = ApkFile::open(apk_path).expect("open for full pipeline failed");
        apk6.manifest_mut().set_version_code(12345);
        apk6.manifest_mut().set_version_name("1.0.0-test");

        let mut patched6 = 0;
        if let Some(dex) = apk6.dex_mut().dex_mut(0) {
            for class in &mut dex.classes {
                if let Some(ref mut data) = class.class_data {
                    for m in data
                        .direct_methods
                        .iter_mut()
                        .chain(data.virtual_methods.iter_mut())
                    {
                        if let Some(ref mut code) = m.code {
                            if code.instructions.len() > 5 && patched6 < 2 {
                                code.return_early();
                                patched6 += 1;
                            }
                        }
                    }
                }
                if patched6 >= 2 {
                    break;
                }
            }
        }

        let tmp6 = tempfile::tempdir().expect("tempdir failed");
        apk6.write_to(tmp6.path()).expect("write_to failed");
        drop(apk6);

        let out6 = find_apk_in_dir(tmp6.path());
        let unsigned6 = std::fs::read(&out6).expect("read failed");

        let key6 = stitch_sign::SigningKey::generate().expect("keygen failed");
        let signed6 =
            stitch_sign::v2::sign(&unsigned6, &key6).expect("signing failed");

        let signed_path = tmp6.path().join("signed.apk");
        std::fs::write(&signed_path, &signed6).expect("write signed failed");
        let apk6r = ApkFile::open(&signed_path).expect("reopen signed failed");

        assert_eq!(apk6r.package_name().map(|s| s.to_owned()), original_package);
        assert_eq!(apk6r.version_code(), Some(12345));
        assert_eq!(apk6r.version_name(), Some("1.0.0-test"));

        eprintln!(
            "  full pipeline OK (dex={} signed={} bytes)",
            apk6r.dex().len(),
            signed6.len(),
        );
    }
}

fn find_apk_in_dir(dir: &std::path::Path) -> std::path::PathBuf {
    for entry in std::fs::read_dir(dir).expect("read dir failed") {
        let entry = entry.expect("entry failed");
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "apk") {
            return path;
        }
    }
    panic!("no APK found in output dir");
}
