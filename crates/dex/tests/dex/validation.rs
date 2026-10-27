// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::{self, TestResult};
use reseam_dex::ParseOptions;

#[test]
fn malformed_files_fail_to_parse() -> TestResult {
    let bytes = common::write(&common::dex())?;
    let header = reseam_dex::parse(&bytes, ParseOptions::default())?
        .header()
        .clone();
    let with_word = |at: usize, value: u32, input: &[u8]| {
        let mut corrupted = input.to_vec();
        corrupted[at..at + 4].copy_from_slice(&value.to_le_bytes());
        corrupted
    };
    let mut malformed: Vec<Vec<u8>> = [0, 8, 111, bytes.len() - 1]
        .into_iter()
        .map(|length| bytes[..length].to_vec())
        .collect();
    let mut doubled = bytes.clone();
    doubled.extend_from_slice(&bytes);
    for count_at in [0x38, 0x40, 0x48, 0x50, 0x58, 0x60] {
        malformed.push(with_word(count_at, u32::MAX, &bytes));
        malformed.push(with_word(count_at + 4, bytes.len() as u32, &doubled));
    }
    for reference_at in [
        header.type_ids_off,
        header.proto_ids_off + 4,
        header.field_ids_off + 4,
        header.method_ids_off + 4,
        header.class_defs_off,
    ] {
        malformed.push(with_word(reference_at as usize, u32::MAX, &bytes));
    }
    for input in malformed {
        assert!(reseam_dex::parse(&input, common::unchecked()).is_err());
    }
    Ok(())
}
