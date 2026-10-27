// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::{self, TestResult};
use reseam_dex::{Loading, ParseOptions};

#[test]
fn android_string_encodings_survive_lookup_interning_and_serialization() -> TestResult {
    let text = ["", "a", "a\0b", "日本語", "𝄞", "\u{ffff}"];
    let mut dex = common::dex();
    for value in text {
        dex.intern_string(value);
    }
    let bytes = common::write(&dex)?;
    for loading in [Loading::Eager, Loading::Deferred] {
        let mut dex = common::parse_with_loading(&bytes, loading)?;
        assert_eq!(common::write(&dex)?, bytes);
        dex.intern_string("!new");
        let output = common::write(&dex)?;
        let result = reseam_dex::parse(&output, ParseOptions::default())?;
        for value in text {
            let index = result.strings().find(value).expect("fixture");
            assert_eq!(result.strings().get(index), value);
        }
    }
    let mut dex =
        reseam_dex::DexFile::new(reseam_dex::DexHeader::new(reseam_dex::DexVersion::V039));
    dex.intern_string("\u{fffd}");
    let mut bytes = common::write(&dex)?;
    let parsed = reseam_dex::parse(&bytes, ParseOptions::default())?;
    let ids = parsed.header().string_ids_off as usize;
    let data = u32::from_le_bytes(bytes[ids..ids + 4].try_into()?) as usize;
    let options = common::unchecked();
    for length in [0, 2, 127] {
        let mut invalid = bytes.clone();
        invalid[data] = length;
        assert!(reseam_dex::parse(&invalid, options).is_err());
    }
    bytes[data + 1..data + 4].copy_from_slice(&[0xed, 0xa0, 0x80]);
    let mut dex = reseam_dex::parse(&bytes, options)?;
    assert!(dex.strings().find("\u{fffd}").is_none());
    dex.intern_string("\u{fffd}");
    let output = common::write(&dex)?;
    let result = reseam_dex::parse(&output, ParseOptions::default())?;
    assert_eq!(result.strings().len(), 2);
    assert!(
        output
            .windows(5)
            .any(|item| item == [1, 0xed, 0xa0, 0x80, 0])
    );
    Ok(())
}
