// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::manifest::manifest_bytes;
use crate::common::session::{entry_data_start_and_compression, lazy};
use reseam_apk::{ApkFile as Apk, Compression};
use std::io::Read;

#[test]
fn opened_entries_decode_as_read_keep_what_they_opened_and_check_their_crc() -> TestResult {
    let name = "assets/model.bin";
    let data: Vec<u8> = (0..3_000_000u32)
        .map(|i| (i % 251) as u8 ^ (i / 4093) as u8)
        .collect();
    for method in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("app.apk");
        let archive = reseam_fixtures::zip_with(
            &[
                ("AndroidManifest.xml", &manifest_bytes("original", None)),
                (name, &data),
            ],
            zip::write::SimpleFileOptions::default().compression_method(method),
        );
        std::fs::write(&path, &archive)?;
        let mut apk = Apk::open(&path, lazy())?;
        let mut opened = apk.open_component_entry(0, name)?.expect("entry");
        apk.inject_file(0, name, b"replaced".to_vec(), Compression::Deflated)?;
        let mut read = Vec::new();
        opened.read_to_end(&mut read)?;
        assert!(
            read == data,
            "{method}: the reader keeps the entry it opened"
        );
        assert_eq!(
            apk.read_component_entry(0, name)?.as_deref(),
            Some(b"replaced".as_slice())
        );

        let (start, _) = entry_data_start_and_compression(&path, name);
        let mut corrupt = archive;
        corrupt[start as usize + 1000] ^= 0xff;
        std::fs::write(&path, corrupt)?;
        assert!(
            Apk::open(&path, lazy())?
                .read_component_entry(0, name)
                .is_err(),
            "{method}: altered data fails its check"
        );
    }
    Ok(())
}
