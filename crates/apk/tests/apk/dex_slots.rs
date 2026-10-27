// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::dex::{container_dex_bytes, dex_bytes};
use crate::common::session::*;
use reseam_apk::{ApkFile as Apk, ApkWriteOptions as WriteOptions, Compression};
use reseam_dex::{DexFile, DexHeader, DexVersion, Loading, ParseOptions};

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
            assert_eq!(apk.dex().len(), 2);
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
            assert!(members[1].strings().find("patched second member").is_some());
            for name in ["edited", "edited-again"] {
                let paths = apk.write_to(dir.path().join(name), WriteOptions::default())?;
                let reopened = Apk::open_split(&paths[0], &paths[1..], options)?;
                assert_eq!(reopened.dex().len(), 2);
                for (index, expected) in ["LFirst;", "LSecond;"].into_iter().enumerate() {
                    let dex = reopened.dex().get(index).expect("DEX");
                    assert_eq!(
                        dex.type_descriptor(dex.classes().header(0).class_type),
                        expected
                    );
                    assert_eq!(
                        dex.strings().find("patched second member").is_some(),
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
        ("classes2.dex", &dex_bytes(&unrelated)),
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
            original.type_descriptor(original.classes().header(0).class_type),
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
        .map(|d| {
            d.type_descriptor(d.classes().header(0).class_type)
                .into_owned()
        })
        .collect();
    assert_eq!(classes, ["LX;", "LY;", "LUnrelated;"]);
    assert_eq!(
        reopened.read_entry("classes2.dex")?,
        Some(dex_bytes(&unrelated))
    );
    apk.delete_file(0, "classes.dex")?;
    for index in [0, 1, 3] {
        assert!(apk.dex_mut(index).is_none());
    }
    assert!(apk.dex_mut(2).is_some());
    let paths = apk.write_to(dir.path().join("deleted"), WriteOptions::default())?;
    let reopened = Apk::open(&paths[0], lazy())?;
    assert_eq!(reopened.dex().len(), 1);
    let dex = reopened.dex().get(0).expect("unrelated DEX");
    assert_eq!(
        dex.type_descriptor(dex.classes().header(0).class_type),
        "LUnrelated;"
    );
    Ok(())
}
