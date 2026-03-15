use stitch_dex::ParseOptions;

#[test]
fn test_parse_truncated_input_returns_error_instead_of_panicking() {
    let mut buf = vec![0; 112];
    buf[..8].copy_from_slice(b"dex\n035\0");
    buf[0x20..0x24].copy_from_slice(&(112u32).to_le_bytes());
    buf[0x24..0x28].copy_from_slice(&(0x70u32).to_le_bytes());
    buf[0x28..0x2C].copy_from_slice(&(0x12345678u32).to_le_bytes());
    buf[0x34..0x38].copy_from_slice(&(200u32).to_le_bytes());

    assert!(matches!(
        stitch_dex::reader::parse(
            &buf,
            ParseOptions {
                skip_checksum: true,
                skip_signature: true,
                ..ParseOptions::default()
            },
        ),
        Err(stitch_dex::DexError::Truncated { .. })
    ));
}
