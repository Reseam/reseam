// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_main]
use libfuzzer_sys::fuzz_target;
use reseam_dex::ParseOptions;

fuzz_target!(|data: &[u8]| {
    let opts = ParseOptions {
        skip_checksum: true,
        skip_signature: true,
        lenient_leb128: true,
        lenient_mutf8: true,
        lazy: false,
    };

    // If parse succeeds, write must not panic
    if let Ok(mut dex) = reseam_dex::parse(data, opts) {
        if let Ok(output) = reseam_dex::write(&mut dex) {
            // Re-parse the output — must not panic
            let opts2 = ParseOptions {
                skip_checksum: true,
                skip_signature: true,
                lenient_leb128: true,
                lenient_mutf8: true,
                lazy: false,
            };
            let _ = reseam_dex::parse(&output, opts2);
        }
    }
});
