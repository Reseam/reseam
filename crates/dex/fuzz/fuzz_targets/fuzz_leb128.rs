// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz ULEB128 decoder — must not panic
    let _ = reseam_dex::encoding::leb128::read_uleb128(data, 0);

    // Fuzz SLEB128 decoder
    let _ = reseam_dex::encoding::leb128::read_sleb128(data, 0);

    // Fuzz ULEB128p1 decoder
    let _ = reseam_dex::encoding::leb128::read_uleb128p1(data, 0);
});
