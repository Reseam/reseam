// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::dex::{class_dex, dex_bytes};
use crate::common::session::*;
use reseam_apk::entry::dex_ordinal;
use reseam_apk::{ApkFile as Apk, ApkWriteOptions as WriteOptions, Compression};
use reseam_dex::ParseOptions;

struct GapCase {
    name: &'static str,
    /// Classes of the app's DEX entries, `classes.dex` first.
    app: &'static [&'static str],
    /// A class written as the next DEX entry during the session.
    injected: Option<&'static str>,
    edited: Option<usize>,
    deleted: &'static str,
    /// Classes of the written DEX entries, `classes.dex` first.
    written: &'static [&'static str],
}

#[test]
fn written_dex_entries_are_numbered_without_gaps() -> TestResult {
    let cases = [
        GapCase {
            name: "a DEX injected and deleted after a DEX was added",
            app: &["LFirst;", "LSecond;"],
            injected: Some("LTemporary;"),
            edited: None,
            deleted: "classes3.dex",
            written: &["LFirst;", "LSecond;", "LAdded;"],
        },
        GapCase {
            name: "an app DEX deleted before unedited and edited ones",
            app: &["LFirst;", "LDeleted;", "LUnedited;", "LEdited;"],
            injected: None,
            edited: Some(3),
            deleted: "classes2.dex",
            written: &["LFirst;", "LUnedited;", "LEdited;", "LAdded;"],
        },
    ];
    for case in cases {
        let app: Vec<(String, Vec<u8>)> = case
            .app
            .iter()
            .enumerate()
            .map(|(index, descriptor)| (dex_entry(index), dex_bytes(&class_dex(descriptor))))
            .collect();
        let entries: Vec<(&str, &[u8])> = app
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .collect();
        let Session { dir, mut apk } = Session::single(&entries);
        if let Some(descriptor) = case.injected {
            let name = dex_entry(case.app.len());
            apk.inject_file(
                0,
                &name,
                dex_bytes(&class_dex(descriptor)),
                Compression::Deflated,
            )?;
        }
        if let Some(index) = case.edited {
            apk.dex_mut(index)
                .expect("live DEX")
                .intern_string("patched");
        }
        apk.add_dex(class_dex("LAdded;"));
        apk.delete_file(0, case.deleted)?;
        let paths = apk.write_to(dir.path().join("out"), WriteOptions::default())?;
        let mut written = Apk::open(&paths[0], lazy())?;
        let mut names: Vec<String> = written
            .entry_names()
            .into_iter()
            .filter(|name| dex_ordinal(name).is_some())
            .collect();
        names.sort_by_key(|name| dex_ordinal(name));
        let expected: Vec<String> = (0..case.written.len()).map(dex_entry).collect();
        assert_eq!(names, expected, "{}", case.name);
        for (name, descriptor) in names.iter().zip(case.written) {
            let bytes = written.read_entry(name)?.expect("listed entry");
            let dex = reseam_dex::parse(&bytes, ParseOptions::default())?;
            assert_eq!(
                dex.type_descriptor(dex.classes().header(0).class_type),
                *descriptor,
                "{}: {name}",
                case.name
            );
        }
    }
    Ok(())
}

/// The name of the DEX entry at zero-based `index`.
fn dex_entry(index: usize) -> String {
    match index {
        0 => "classes.dex".to_owned(),
        index => format!("classes{}.dex", index + 1),
    }
}
