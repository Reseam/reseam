// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::{entry, open_split_test_apk};
use crate::common::bundles::apply_fixture;
use reseam_apk::ApkWriteOptions;

#[test]
fn files_streamed_in_and_out_match_files_written_whole() {
    let large: Vec<u8> = (0..3 * 1024 * 1024 + 17u32)
        .map(|i| (i % 251) as u8 ^ (i / 4093) as u8)
        .collect();
    let (dir, mut apk) = open_split_test_apk();
    apply_fixture(&mut apk, "file-streams", "fileStreams");

    assert_eq!(
        entry(&mut apk, 0, "assets/large.bin"),
        Some(b"replaced".to_vec())
    );
    assert!(
        entry(&mut apk, 0, "assets/large-stored.bin").is_some_and(|bytes| bytes == large),
        "a file streamed in reads back whole"
    );
    assert!(
        reseam_apk::axml::is_compiled_axml(&entry(&mut apk, 0, "res/xml/streamed.xml").unwrap()),
        "streamed XML under res/ compiles like XML written whole"
    );
    assert_eq!(entry(&mut apk, 0, "assets/failed.bin"), None);

    let written = apk
        .write_to(dir.path().join("out"), ApkWriteOptions::default())
        .unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&written[0]).unwrap()).unwrap();
    for (name, method) in [
        ("assets/large-stored.bin", zip::CompressionMethod::Stored),
        ("assets/large.bin", zip::CompressionMethod::Deflated),
    ] {
        assert_eq!(
            archive.by_name(name).unwrap().compression(),
            method,
            "{name}"
        );
    }
}
