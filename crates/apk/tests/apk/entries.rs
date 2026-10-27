// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::dex::{dex_bytes, empty_dex, minimal_dex_bytes};
use crate::common::manifest::manifest_bytes;
use crate::common::session::*;
use reseam_apk::{
    ApkFile as Apk, ApkWriteOptions as WriteOptions, Compression, ResValue, ResourceTable,
};

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
fn replacing_format_entries_replaces_their_parsed_state_before_further_edits() -> TestResult {
    for edit in [false, true] {
        let Session { dir, mut apk } = Session::single(&[
            ("classes.dex", &minimal_dex_bytes()),
            ("resources.arsc", &mutable_resource_table_bytes()),
        ]);
        apk.dex_mut(0).expect("DEX").intern_string("discarded");
        apk.base_mut().manifest_mut().set_version_name("discarded");
        let resources = apk.base_mut().resources_mut()?.expect("table");
        let discarded = ResValue::string(resources.add_global_string("discarded"));
        resources.add_resource("string", "greeting", discarded)?;
        let mut dex = empty_dex();
        dex.intern_string("replacement");
        let mut table =
            ResourceTable::parse(reseam_apk::Bytes::from_vec(mutable_resource_table_bytes()))?;
        let replacement = ResValue::string(table.add_global_string("replacement"));
        table.add_resource("string", "greeting", replacement)?;
        for (name, bytes) in [
            ("AndroidManifest.xml", manifest_bytes("replacement", None)),
            ("classes.dex", dex_bytes(&dex)),
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
            let dex = reopened.dex().get(0).expect("DEX");
            assert!(dex.strings().find(expected).is_some());
            assert!(dex.strings().find("discarded").is_none());
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
            .get(1)
            .expect("DEX")
            .strings()
            .find("retained")
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
            .get(0)
            .expect("DEX")
            .strings()
            .find("retained")
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
