use criterion::{criterion_group, criterion_main, Criterion};
use stitch_dex::ParseOptions;

fn bench_parse_minimal(c: &mut Criterion) {
    // Construct a minimal valid DEX header for benchmarking parse error paths.
    // Full APK-based benchmarks live in stitch-apk.
    let mut buf = vec![0u8; 112];
    buf[..8].copy_from_slice(b"dex\n035\0");

    c.bench_function("parse_invalid_header", |b| {
        b.iter(|| {
            let _ = stitch_dex::parse(
                &buf,
                ParseOptions {
                    skip_checksum: true,
                    skip_signature: true,
                    ..ParseOptions::default()
                },
            );
        });
    });
}

criterion_group!(benches, bench_parse_minimal);
criterion_main!(benches);
