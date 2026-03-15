#![no_main]
use libfuzzer_sys::fuzz_target;
use stitch_dex::ParseOptions;

fuzz_target!(|data: &[u8]| {
    let opts = ParseOptions {
        skip_checksum: true,
        skip_signature: true,
        lenient_leb128: true,
        lenient_mutf8: true,
        lazy: false,
    };

    // If parse succeeds, write must not panic
    if let Ok(dex) = stitch_dex::reader::parse(data, opts) {
        if let Ok(output) = stitch_dex::writer::write(&dex) {
            // Re-parse the output — must not panic
            let opts2 = ParseOptions {
                skip_checksum: true,
                skip_signature: true,
                lenient_leb128: true,
                lenient_mutf8: true,
                lazy: false,
            };
            let _ = stitch_dex::reader::parse(&output, opts2);
        }
    }
});
