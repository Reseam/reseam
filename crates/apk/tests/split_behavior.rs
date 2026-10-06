// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

#[path = "common/archive.rs"]
mod archive;
#[path = "common/dex.rs"]
mod dex_fixture;
#[path = "common/manifest.rs"]
mod manifest;
#[path = "common/session.rs"]
mod session;
use dex_fixture::{container_dex_bytes, minimal_dex_bytes};
use manifest::manifest_bytes;
use reseam_apk::{ApkFile as Apk, ApkWriteOptions as WriteOptions, Compression, ResourceTable};
use reseam_dex::{DexFile, DexHeader, DexVersion, Loading, ParseOptions};
use session::*;
use std::fs::File;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn split_apk_supports_split_resource_tables_and_component_state() -> TestResult {
    let Session { dir, mut apk } = Session::split(
        &[("assets/old.txt", b"old-base")],
        &[
            ("resources.arsc", &resource_table_bytes()),
            ("assets/old.txt", b"old-split"),
        ],
        lazy(),
    );
    assert_eq!(apk.components().len(), 2);
    assert_eq!(apk.components()[1].name(), "feature");
    assert_eq!(
        component_state(&mut apk, 1)?,
        (Some("1.0-split".into()), true)
    );
    apk.component_mut(1)
        .expect("split")
        .manifest_mut()
        .set_version_name("2.0-split");
    for (component, name, bytes) in [
        (0, "assets/new.txt", b"new-base".as_slice()),
        (1, "assets/split-only.txt", b"new-split".as_slice()),
    ] {
        apk.inject_file(component, name, bytes.to_vec(), Compression::Deflated)?;
    }
    apk.delete_file(0, "assets/old.txt")?;
    let paths = apk.write_to(dir.path().join("out"), WriteOptions::default())?;
    let mut reopened = Apk::open_split(&paths[0], &paths[1..], lazy())?;
    assert_eq!(
        component_state(&mut reopened, 1)?,
        (Some("2.0-split".into()), true)
    );
    for component in 0..2 {
        for (name, expected) in [
            ("assets/new.txt", component == 0),
            ("assets/old.txt", component == 1),
            ("assets/split-only.txt", component == 1),
        ] {
            assert_eq!(
                reopened
                    .component(component)
                    .expect("component")
                    .contains(name),
                expected
            );
        }
    }
    Ok(())
}

#[test]
fn write_to_applies_the_signature_policy_without_removing_other_metadata() -> TestResult {
    use reseam_apk::SignaturePolicy;
    let entries: Vec<_> = SIGNATURES
        .iter()
        .chain(&METADATA)
        .map(|&name| (name, b"payload".as_slice()))
        .collect();
    let Session { dir, mut apk } = Session::single(&entries);
    apk.base_mut().manifest_mut().set_version_name("edited");
    for (policy, retained) in [
        (SignaturePolicy::Strip, false),
        (SignaturePolicy::Preserve, true),
    ] {
        let output = apk.write_to(
            dir.path().join("out"),
            WriteOptions {
                signatures: policy,
                ..Default::default()
            },
        )?;
        let mut reopened = Apk::open(&output[0], lazy())?;
        for name in SIGNATURES.iter().chain(&METADATA) {
            assert_eq!(
                reopened.read_entry(name)?.as_deref(),
                (retained || METADATA.contains(name)).then_some(b"payload".as_slice()),
                "{name}"
            );
        }
    }
    Ok(())
}

#[test]
fn replacing_format_entries_replaces_their_parsed_state_before_further_edits() -> TestResult {
    for edit in [false, true] {
        let Session { dir, mut apk } = Session::single(&[
            ("classes.dex", &minimal_dex_bytes()),
            ("resources.arsc", &mutable_resource_table_bytes()),
        ]);
        apk.dex_mut(0).expect("DEX").intern_string("discarded");
        apk.base_mut().manifest_mut().set_version_name("discarded");
        apk.base_mut()
            .resources_mut()?
            .expect("table")
            .add_string_resource("greeting", "discarded")?;
        let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
        dex.intern_string("replacement");
        let mut table =
            ResourceTable::parse(reseam_apk::Bytes::from_vec(mutable_resource_table_bytes()))?;
        table.add_string_resource("greeting", "replacement")?;
        for (name, bytes) in [
            ("AndroidManifest.xml", manifest_bytes("replacement", None)),
            ("classes.dex", reseam_dex::write(&dex)?),
            ("resources.arsc", table.serialize()?),
        ] {
            apk.inject_file(0, name, bytes.clone(), Compression::Stored)?;
            assert_eq!(apk.read_component_entry(0, name)?, Some(bytes));
        }
        assert_eq!(apk.version_name().as_deref(), Some("replacement"));
        assert_eq!(
            apk.string_resource("greeting")?.as_deref(),
            Some("replacement")
        );
        if edit {
            apk.base_mut().manifest_mut().set_version_name("edited");
            apk.set_string_resource("greeting", "edited")?;
            apk.dex_mut(0).expect("DEX").intern_string("edited");
        }
        let expected = if edit { "edited" } else { "replacement" };
        for name in ["first", "second"] {
            let output = apk.write_to(dir.path().join(name), WriteOptions::default())?;
            let mut reopened = Apk::open(&output[0], lazy())?;
            assert_eq!(reopened.version_name().as_deref(), Some(expected));
            assert_eq!(
                reopened.string_resource("greeting")?.as_deref(),
                Some(expected)
            );
            let dex = reopened.dex().dex(0).expect("DEX");
            assert!(dex.find_string_idx(expected).is_some());
            assert!(dex.find_string_idx("discarded").is_none());
        }
    }
    Ok(())
}

#[test]
fn deletion_removes_dirty_entries_and_keeps_other_dex_identities_stable() -> TestResult {
    let Session { dir, mut apk } = Session::single(&[
        ("classes.dex", &minimal_dex_bytes()),
        ("classes2.dex", &minimal_dex_bytes()),
        ("resources.arsc", &mutable_resource_table_bytes()),
    ]);
    apk.dex_mut(0).expect("DEX").intern_string("deleted");
    apk.dex_mut(1).expect("DEX").intern_string("retained");
    apk.set_string_resource("greeting", "deleted")?;
    for name in ["resources.arsc", "classes.dex"] {
        apk.delete_file(0, name)?;
        assert!(apk.read_entry(name)?.is_none());
        assert!(!apk.entry_names().iter().any(|entry| entry == name));
    }
    assert!(!apk.base().has_resources());
    assert!(apk.dex_mut(0).is_none());
    assert!(
        apk.dex()
            .dex(1)
            .expect("DEX")
            .find_string_idx("retained")
            .is_some()
    );
    assert!(apk.delete_file(0, "AndroidManifest.xml").is_err());
    let paths = apk.write_to(dir.path().join("out"), WriteOptions::default())?;
    let reopened = Apk::open(&paths[0], lazy())?;
    assert!(!reopened.base().contains("classes.dex"));
    assert!(!reopened.base().contains("resources.arsc"));
    assert!(
        reopened
            .dex()
            .dex(0)
            .expect("DEX")
            .find_string_idx("retained")
            .is_some()
    );
    Ok(())
}

#[test]
fn failed_replacements_and_publication_leave_the_input_usable() -> TestResult {
    let Session { dir, mut apk } = Session::single(&[
        ("classes.dex", &minimal_dex_bytes()),
        ("resources.arsc", &mutable_resource_table_bytes()),
    ]);
    let original = std::fs::read(dir.path().join("app.apk"))?;
    for name in ["AndroidManifest.xml", "classes.dex", "resources.arsc"] {
        let before = apk.read_entry(name)?;
        assert!(
            apk.inject_file(0, name, b"invalid format".to_vec(), Compression::Stored)
                .is_err()
        );
        assert_eq!(apk.read_entry(name)?, before);
    }
    apk.base_mut().manifest_mut().set_version_name("edited");
    assert!(apk.write_to(dir.path(), WriteOptions::default()).is_err());
    assert_eq!(std::fs::read(dir.path().join("app.apk"))?, original);
    assert_eq!(apk.version_name().as_deref(), Some("edited"));
    Ok(())
}

#[test]
fn passthrough_and_injected_native_libraries_are_stored_aligned_and_unchanged() -> TestResult {
    let Session { dir, mut apk } = Session::single(&[
        ("assets/raw.bin", b"asset"),
        ("lib/arm64-v8a/liboriginal.so", b"\x7fELF original"),
    ]);
    for name in ["lib/arm64-v8a/libinjected.so", "lib/arm64-v8a/.so"] {
        apk.inject_file(0, name, b"\x7fELF injected".to_vec(), Compression::Deflated)?;
    }
    let output = apk.write_to(dir.path().join("out"), WriteOptions::default())?;
    let mut reopened = Apk::open(&output[0], lazy())?;
    for (name, alignment, bytes) in LIBRARIES {
        let (start, compression) = entry_data_start_and_compression(&output[0], name);
        assert_eq!(start % alignment, 0, "{name}");
        assert_eq!(compression, zip::CompressionMethod::Stored);
        assert_eq!(reopened.read_entry(name)?.as_deref(), Some(bytes));
    }
    Ok(())
}

#[test]
fn manifest_edits_preserve_untouched_payload_bytes_and_archive_metadata() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("app.apk");
    let entries = [
        ("classes.dex", minimal_dex_bytes()),
        ("resources.arsc", mutable_resource_table_bytes()),
    ];
    let time = zip::DateTime::from_date_and_time(2004, 5, 6, 7, 8, 10)?;
    for compression in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(compression)
            .last_modified_time(time);
        let manifest = manifest_bytes("base", None);
        let packed: Vec<_> = std::iter::once(("AndroidManifest.xml", manifest.as_slice()))
            .chain(
                entries
                    .iter()
                    .map(|(name, bytes)| (*name, bytes.as_slice())),
            )
            .collect();
        archive::write_with_options(&path, &packed, options);
        let mut apk = Apk::open(&path, lazy())?;
        apk.base_mut().manifest_mut().set_version_name("edited");
        let output = apk.write_to(dir.path().join("out"), WriteOptions::default())?;
        let mut zip = zip::ZipArchive::new(File::open(&output[0])?)?;
        for (name, expected) in &entries {
            let mut entry = zip.by_name(name)?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes)?;
            assert_eq!(bytes, *expected);
            assert_eq!(entry.compression(), compression);
            assert_eq!(entry.last_modified(), Some(time));
        }
    }
    Ok(())
}

#[test]
fn stored_entry_ranges_must_fit_the_archive_before_mapping() -> TestResult {
    let Session { dir, apk: _ } = Session::single(&[("classes.dex", &minimal_dex_bytes())]);
    let path = dir.path().join("app.apk");
    let mut bytes = std::fs::read(&path)?;
    let central = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(i, sig)| (sig == b"PK\x01\x02").then_some(i))
        .find(|&i| {
            let len = u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]) as usize;
            &bytes[i + 46..i + 46 + len] == b"classes.dex"
        })
        .expect("DEX record");
    for offset in [20, 24] {
        bytes[central + offset..central + offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    std::fs::write(&path, bytes)?;
    assert!(Apk::open(&path, lazy()).is_err());
    Ok(())
}

#[test]
fn dex_containers_preserve_all_members_through_loading_and_editing() -> TestResult {
    for loading in [Loading::Eager, Loading::Deferred] {
        for component in [0, 1] {
            let bytes = container_dex_bytes(&["LFirst;", "LSecond;"]);
            let entries = [("classes.dex", bytes.as_slice())];
            let options = ParseOptions {
                classes: loading,
                ..Default::default()
            };
            let Session { dir, mut apk } = Session::split(
                if component == 0 { &entries } else { &[] },
                if component == 1 { &entries } else { &[] },
                options,
            );
            let input = dir.path().join(if component == 0 {
                "base.apk"
            } else {
                "split.apk"
            });
            let (extracted, names) = reseam_apk::extract_dex(&input, options)?;
            assert_eq!(names, ["classes.dex", "classes.dex"]);
            assert_eq!(extracted.len(), 2);
            let paths = apk.write_to(dir.path().join("unchanged"), WriteOptions::default())?;
            let mut unchanged = Apk::open_split(&paths[0], &paths[1..], options)?;
            assert_eq!(
                unchanged.read_component_entry(component, "classes.dex")?,
                Some(bytes)
            );
            apk.dex_mut(1)
                .expect("DEX")
                .intern_string("patched second member");
            let edited = apk
                .read_component_entry(component, "classes.dex")?
                .expect("container");
            let members = reseam_dex::parse_container(&edited, options)?;
            assert_eq!(members.len(), 2);
            assert!(
                members[1]
                    .find_string_idx("patched second member")
                    .is_some()
            );
            for name in ["edited", "edited-again"] {
                let paths = apk.write_to(dir.path().join(name), WriteOptions::default())?;
                let reopened = Apk::open_split(&paths[0], &paths[1..], options)?;
                assert_eq!(reopened.dex().len(), 2);
                for (index, expected) in ["LFirst;", "LSecond;"].into_iter().enumerate() {
                    let dex = reopened.dex().dex(index).expect("DEX");
                    assert_eq!(
                        dex.type_descriptor(dex.class_header(0).class_type),
                        expected
                    );
                    assert_eq!(
                        dex.find_string_idx("patched second member").is_some(),
                        index == 1
                    );
                }
                assert!(
                    reopened
                        .components()
                        .iter()
                        .all(|c| !c.contains("classes2.dex"))
                );
            }
        }
    }
    Ok(())
}

#[test]
fn replacing_and_deleting_dex_containers_keeps_unrelated_slots_stable() -> TestResult {
    let mut unrelated = DexFile::new(DexHeader::new(DexVersion::V039));
    unrelated.create_class("LUnrelated;", reseam_dex::AccessFlags::PUBLIC, None)?;
    let Session { dir, mut apk } = Session::single(&[
        ("classes.dex", &container_dex_bytes(&["LA;", "LB;"])),
        ("classes2.dex", &reseam_dex::write(&unrelated)?),
    ]);
    for classes in [
        vec!["LSingle;"],
        vec!["LA;", "LB;", "LC;"],
        vec!["LX;", "LY;"],
    ] {
        let bytes = container_dex_bytes(&classes);
        apk.inject_file(0, "classes.dex", bytes.clone(), Compression::Deflated)?;
        assert_eq!(apk.read_entry("classes.dex")?, Some(bytes));
        let original = apk.dex_mut(2).expect("unrelated DEX");
        assert_eq!(
            original.type_descriptor(original.class_header(0).class_type),
            "LUnrelated;"
        );
        assert_eq!(
            apk.dex().iter().filter(|d| !d.classes().is_empty()).count(),
            classes.len() + 1
        );
        let before = apk.read_entry("classes.dex")?;
        assert!(
            apk.inject_file(
                0,
                "classes.dex",
                b"dex\n041\0truncated".to_vec(),
                Compression::Stored
            )
            .is_err()
        );
        assert_eq!(apk.read_entry("classes.dex")?, before);
    }
    apk.dex_mut(1).expect("DEX").intern_string("edited member");
    let paths = apk.write_to(dir.path().join("edited"), WriteOptions::default())?;
    let mut reopened = Apk::open(&paths[0], lazy())?;
    let classes: Vec<_> = reopened
        .dex()
        .iter()
        .map(|d| d.type_descriptor(d.class_header(0).class_type).into_owned())
        .collect();
    assert_eq!(classes, ["LX;", "LY;", "LUnrelated;"]);
    assert_eq!(
        reopened.read_entry("classes2.dex")?,
        Some(reseam_dex::write(&unrelated)?)
    );
    apk.delete_file(0, "classes.dex")?;
    for index in [0, 1, 3] {
        assert!(apk.dex_mut(index).is_none());
    }
    assert!(apk.dex_mut(2).is_some());
    let paths = apk.write_to(dir.path().join("deleted"), WriteOptions::default())?;
    let reopened = Apk::open(&paths[0], lazy())?;
    assert_eq!(reopened.dex().len(), 1);
    let dex = reopened.dex().dex(0).expect("unrelated DEX");
    assert_eq!(
        dex.type_descriptor(dex.class_header(0).class_type),
        "LUnrelated;"
    );
    Ok(())
}

#[test]
fn serialization_failure_preserves_destinations_and_allows_repair() -> TestResult {
    for workers in [1, 4] {
        for invalid_component in [0, 1] {
            let entries = [("classes.dex", minimal_dex_bytes())];
            let entries = entries
                .each_ref()
                .map(|(name, data)| (*name, data.as_slice()));
            let Session { dir, mut apk } = Session::split(&entries, &entries, lazy());
            let output = dir.path().join("out");
            std::fs::create_dir(&output)?;
            let mut before = Vec::new();
            for name in ["base.apk", "split.apk"] {
                std::fs::copy(dir.path().join(name), output.join(name))?;
                before.push(std::fs::read(output.join(name))?);
            }
            apk.base_mut().manifest_mut().set_version_name("edited");
            for index in 0..2 {
                apk.dex_mut(index).expect("DEX").intern_string("edited");
            }
            for _ in 0..2 {
                apk.dex_mut(invalid_component).expect("DEX").create_class(
                    "LRepeated;",
                    reseam_dex::AccessFlags::PUBLIC,
                    None,
                )?;
            }
            let options = WriteOptions {
                dex_workers: std::num::NonZeroUsize::new(workers).expect("positive workers"),
                ..Default::default()
            };
            assert!(apk.write_to(&output, options).is_err());
            for (name, bytes) in ["base.apk", "split.apk"].into_iter().zip(before) {
                assert_eq!(std::fs::read(output.join(name))?, bytes);
            }
            let invalid = apk.dex_mut(invalid_component).expect("DEX");
            let repeated = invalid.intern_type("LRepeated;")?;
            invalid.remove_class(repeated)?.expect("duplicate class");
            let paths = apk.write_to(&output, options)?;
            let repaired = Apk::open_split(&paths[0], &paths[1..], lazy())?;
            assert_eq!(repaired.version_name().as_deref(), Some("edited"));
            assert_eq!(
                repaired
                    .dex()
                    .dex(invalid_component)
                    .expect("DEX")
                    .classes()
                    .len(),
                1
            );
            assert!(
                repaired
                    .dex()
                    .iter()
                    .all(|d| d.find_string_idx("edited").is_some())
            );
        }
    }
    Ok(())
}
