// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_main]
use libfuzzer_sys::fuzz_target;
use reseam_dex::ParseOptions;

fuzz_target!(|data: &[u8]| {
    // Parse with strict options — must not panic on any input
    let opts = ParseOptions {
        skip_checksum: true,
        skip_signature: true,
        lenient_leb128: false,
        lenient_mutf8: false,
        lazy: false,
    };
    let _ = reseam_dex::parse(data, opts);
});
