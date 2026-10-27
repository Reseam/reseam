// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::dex::{dex_bytes, empty_dex, minimal_dex_bytes};
use crate::common::manifest::manifest_bytes;
use crate::common::session::{lazy, mutable_resource_table_bytes};
use reseam_apk::{ApkFile as Apk, ApkWriteOptions as WriteOptions, Compression};
use reseam_journal::Savepoint;
use std::fs::File;

#[test]
fn rollback_returns_the_session_to_each_savepoint() -> TestResult {
    type Entries = Result<Vec<(String, Vec<u8>)>, Box<dyn std::error::Error>>;

    let dir = tempfile::tempdir()?;
    let dex = minimal_dex_bytes();
    std::fs::write(
        dir.path().join("app.apk"),
        reseam_fixtures::apk(
            &manifest_bytes("original", None),
            &[
                ("classes.dex", &dex),
                ("classes2.dex", &dex),
                ("resources.arsc", &mutable_resource_table_bytes()),
                ("assets/kept.txt", b"kept"),
            ],
        ),
    )
    .expect("fixture APK");
    let mut apk = Apk::open(dir.path().join("app.apk"), lazy())?;
    let mut written = 0;
    let mut entries = |apk: &mut Apk| -> Entries {
        written += 1;
        let paths = apk.write_to(
            dir.path().join(written.to_string()),
            WriteOptions::default(),
        )?;
        let mut archive = zip::ZipArchive::new(File::open(&paths[0])?)?;
        let mut entries = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            if entry.name().starts_with("META-INF/") {
                continue;
            }
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes)?;
            entries.push((entry.name().to_owned(), bytes));
        }
        Ok(entries)
    };
    let original = entries(&mut apk)?;

    let outer = Savepoint::FIRST;
    apk.open_savepoint(outer);
    apk.base_mut().manifest_mut().set_version_name("outer");
    apk.set_string_resource("greeting", "outer")?;
    apk.dex_mut(0).expect("DEX").intern_string("outer");
    apk.inject_file(
        0,
        "assets/added.txt",
        b"added".to_vec(),
        Compression::Deflated,
    )?;
    let after_outer = entries(&mut apk)?;

    let inner = outer.next();
    apk.open_savepoint(inner);
    apk.delete_file(0, "assets/kept.txt")?;
    apk.delete_file(0, "classes2.dex")?;
    let mut replacement = empty_dex();
    replacement.intern_string("replacement");
    apk.inject_file(
        0,
        "classes.dex",
        dex_bytes(&replacement),
        Compression::Stored,
    )?;
    apk.add_dex(empty_dex());
    apk.inject_file(
        0,
        "AndroidManifest.xml",
        manifest_bytes("injected", None),
        Compression::Stored,
    )?;
    apk.set_string_resource("greeting", "inner")?;
    let released = inner.next();
    apk.open_savepoint(released);
    apk.dex_mut(0).expect("DEX").intern_string("released");
    apk.release_savepoint(released);

    let revision = apk.manifest_revision();
    let returned = apk.rollback_to(inner);
    assert_eq!(returned.len(), 1);
    let readded = inner.next().next();
    apk.open_savepoint(readded);
    let (_, dex) = returned.into_iter().next().expect("the added DEX returns");
    apk.add_dex(dex);
    let returned = apk.rollback_to(readded);
    assert_eq!(returned.len(), 1);
    assert!(apk.manifest_revision() < revision);
    assert_eq!(entries(&mut apk)?, after_outer);
    assert!(apk.rollback_to(outer).is_empty());
    assert_eq!(apk.manifest_revision(), 0);
    assert_eq!(entries(&mut apk)?, original);
    Ok(())
}
