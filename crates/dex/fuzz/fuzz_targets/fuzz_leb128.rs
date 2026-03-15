#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz ULEB128 decoder — must not panic
    let _ = stitch_dex::encoding::leb128::read_uleb128(data, 0);

    // Fuzz SLEB128 decoder
    let _ = stitch_dex::encoding::leb128::read_sleb128(data, 0);

    // Fuzz ULEB128p1 decoder
    let _ = stitch_dex::encoding::leb128::read_uleb128p1(data, 0);
});
