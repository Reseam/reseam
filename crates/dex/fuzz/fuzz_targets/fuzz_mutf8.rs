#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz MUTF-8 decode — must not panic
    if let Ok(s) = stitch_dex::encoding::mutf8::decode_mutf8(data) {
        // If decode succeeds, encode must not panic and should round-trip
        let encoded = stitch_dex::encoding::mutf8::encode_mutf8(&s);
        let _ = stitch_dex::encoding::mutf8::decode_mutf8(&encoded);
    }
});
