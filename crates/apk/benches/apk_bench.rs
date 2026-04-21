// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use reseam_dex::{InstructionPattern, OpcodeMatcher, ParseOptions};
use std::io::Read;

const YOUTUBE_APK: &str = "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";
const INSTAGRAM_APK: &str = "../../test-apks/com.instagram.android_419.0.0.49.71-382508603_minAPI28(arm64-v8a)(360,400,420,480dpi)_apkmirror.com.apk";

fn default_opts() -> ParseOptions {
    ParseOptions::default()
}

fn skip_verify_opts() -> ParseOptions {
    ParseOptions {
        skip_checksum: true,
        skip_signature: true,
        ..ParseOptions::default()
    }
}

fn extract_dex_files_from_apk(apk_path: &str) -> Vec<(String, Vec<u8>)> {
    let file = std::fs::File::open(apk_path).expect("Failed to open APK");
    let mut archive = zip::ZipArchive::new(file).expect("Failed to read ZIP");
    let mut dex_files = Vec::new();
    let mut names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let name = archive.by_index(i).ok()?.name().to_string();
            if name.ends_with(".dex") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    names.sort();
    for name in &names {
        let mut entry = archive.by_name(name).unwrap();
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).unwrap();
        dex_files.push((name.clone(), buf));
    }
    dex_files
}

fn bench_parse(c: &mut Criterion) {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        return;
    }
    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let mut group = c.benchmark_group("parse");
    for (name, buf) in &dex_files {
        group.bench_with_input(BenchmarkId::new("full_parse", name), buf, |b, buf| {
            b.iter(|| reseam_dex::parse(buf, default_opts()).unwrap());
        });
    }
    let (name, buf) = &dex_files[0];
    group.bench_with_input(
        BenchmarkId::new("parse_skip_verify", name),
        buf,
        |b, buf| {
            b.iter(|| reseam_dex::parse(buf, skip_verify_opts()).unwrap());
        },
    );
    let total_size: usize = dex_files.iter().map(|(_, b)| b.len()).sum();
    group.bench_function(
        BenchmarkId::new("all_dex_files", format!("{}MB", total_size / (1024 * 1024))),
        |b| {
            b.iter(|| {
                for (_, buf) in &dex_files {
                    reseam_dex::parse(buf, skip_verify_opts()).unwrap();
                }
            });
        },
    );
    group.finish();
}

fn bench_write(c: &mut Criterion) {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        return;
    }
    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let mut group = c.benchmark_group("write");
    let parsed: Vec<_> = dex_files
        .iter()
        .map(|(name, buf)| {
            (
                name.clone(),
                reseam_dex::parse(buf, default_opts()).unwrap(),
            )
        })
        .collect();
    for (name, dex) in &parsed {
        group.bench_with_input(BenchmarkId::new("write", name), dex, |b, dex| {
            b.iter_batched(
                || dex.clone(),
                |mut d| reseam_dex::write(&mut d).unwrap(),
                criterion::BatchSize::LargeInput,
            );
        });
    }
    group.bench_function("write_all", |b| {
        b.iter_batched(
            || parsed.iter().map(|(_, d)| d.clone()).collect::<Vec<_>>(),
            |mut ds| {
                for d in &mut ds {
                    reseam_dex::write(d).unwrap();
                }
            },
            criterion::BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn bench_round_trip(c: &mut Criterion) {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        return;
    }
    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let (name, buf) = &dex_files[0];
    c.bench_function(&format!("round_trip/{}", name), |b| {
        b.iter(|| {
            let mut dex = reseam_dex::parse(buf, skip_verify_opts()).unwrap();
            let output = reseam_dex::write(&mut dex).unwrap();
            std::hint::black_box(output.len());
        });
    });
}

fn bench_search(c: &mut Criterion) {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        return;
    }
    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let (_, buf) = &dex_files[0];
    let dex = reseam_dex::parse(buf, default_opts()).unwrap();
    let mut group = c.benchmark_group("search");
    group.bench_function("find_method_by_name", |b| {
        b.iter(|| {
            dex.find_method_by(|method_id, _class, _em| dex.string(method_id.name) == "onCreate")
        });
    });
    group.bench_function("find_all_init_methods", |b| {
        b.iter(|| {
            dex.find_methods_by(|method_id, _class, _em| dex.string(method_id.name) == "<init>")
        });
    });
    group.bench_function("find_class", |b| {
        b.iter(|| dex.find_class("Ljava/lang/Object;"));
    });
    group.bench_function("find_methods_with_opcodes", |b| {
        let pattern = [
            InstructionPattern::Opcode(OpcodeMatcher::Const),
            InstructionPattern::Opcode(OpcodeMatcher::Return),
        ];
        b.iter(|| dex.find_methods_with_opcodes(&pattern));
    });
    group.bench_function("find_methods_wildcard_pattern", |b| {
        let pattern = [
            InstructionPattern::Opcode(OpcodeMatcher::InvokeVirtual),
            InstructionPattern::Any,
            InstructionPattern::Opcode(OpcodeMatcher::ReturnObject),
        ];
        b.iter(|| dex.find_methods_with_opcodes(&pattern));
    });
    group.finish();
}

fn bench_mutation(c: &mut Criterion) {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        return;
    }
    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let (_, buf) = &dex_files[0];
    let dex = reseam_dex::parse(buf, default_opts()).unwrap();
    let mut group = c.benchmark_group("mutation");
    group.bench_function("intern_string_new", |b| {
        b.iter_batched(
            || dex.clone(),
            |mut dex| {
                dex.intern_string("Lcom/benchmark/NewClass;");
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.bench_function("intern_string_existing", |b| {
        b.iter_batched(
            || dex.clone(),
            |mut dex| {
                dex.intern_string("Ljava/lang/Object;");
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.bench_function("intern_method", |b| {
        b.iter_batched(
            || dex.clone(),
            |mut dex| {
                dex.intern_method("Lcom/benchmark/Test;", "doSomething", "(II)V")
                    .expect("valid");
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.bench_function("return_early", |b| {
        b.iter_batched(
            || {
                let mut d = dex.clone();
                for class in &mut d.classes {
                    if let Some(ref mut data) = class.class_data {
                        for m in data.direct_methods.iter_mut() {
                            if m.code.is_some() {
                                return d;
                            }
                        }
                    }
                }
                d
            },
            |mut dex| {
                for class in &mut dex.classes {
                    if let Some(ref mut data) = class.class_data {
                        for m in data.direct_methods.iter_mut() {
                            if let Some(ref mut code) = m.code {
                                code.return_early();
                                return;
                            }
                        }
                    }
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_multi_dex(c: &mut Criterion) {
    if !std::path::Path::new(YOUTUBE_APK).exists() {
        return;
    }
    let dex_files = extract_dex_files_from_apk(YOUTUBE_APK);
    let buffers: Vec<&[u8]> = dex_files.iter().map(|(_, b)| b.as_slice()).collect();
    c.bench_function("multi_dex/parse_all", |b| {
        b.iter(|| reseam_dex::MultiDexContainer::parse(&buffers, skip_verify_opts()).unwrap());
    });
    let container = reseam_dex::MultiDexContainer::parse(&buffers, default_opts()).unwrap();
    c.bench_function("multi_dex/write_all", |b| {
        b.iter_batched(
            || container.clone(),
            |mut c| c.write_all().unwrap(),
            criterion::BatchSize::LargeInput,
        );
    });
    c.bench_function("multi_dex/find_class_across_dexes", |b| {
        b.iter(|| container.find_class("Landroid/support/v4/app/Fragment;"));
    });
}

fn bench_instagram(c: &mut Criterion) {
    if !std::path::Path::new(INSTAGRAM_APK).exists() {
        return;
    }
    let dex_files = extract_dex_files_from_apk(INSTAGRAM_APK);
    let total_size: usize = dex_files.iter().map(|(_, b)| b.len()).sum();
    let mut group = c.benchmark_group("instagram");
    let (largest_name, largest_buf) = dex_files.iter().max_by_key(|(_, b)| b.len()).unwrap();
    group.bench_with_input(
        BenchmarkId::new(
            "parse_largest",
            format!("{} ({}MB)", largest_name, largest_buf.len() / (1024 * 1024)),
        ),
        largest_buf,
        |b, buf| {
            b.iter(|| reseam_dex::parse(buf, skip_verify_opts()).unwrap());
        },
    );
    group.bench_function(
        BenchmarkId::new("parse_all", format!("{}MB", total_size / (1024 * 1024))),
        |b| {
            b.iter(|| {
                for (_, buf) in &dex_files {
                    reseam_dex::parse(buf, skip_verify_opts()).unwrap();
                }
            });
        },
    );
    let largest_dex = reseam_dex::parse(largest_buf, default_opts()).unwrap();
    group.bench_function(BenchmarkId::new("write_largest", largest_name), |b| {
        b.iter_batched(
            || largest_dex.clone(),
            |mut d| reseam_dex::write(&mut d).unwrap(),
            criterion::BatchSize::LargeInput,
        );
    });
    group.bench_function(BenchmarkId::new("round_trip_largest", largest_name), |b| {
        b.iter(|| {
            let mut dex = reseam_dex::parse(largest_buf, skip_verify_opts()).unwrap();
            let out = reseam_dex::write(&mut dex).unwrap();
            std::hint::black_box(out.len());
        });
    });
    let buffers: Vec<&[u8]> = dex_files.iter().map(|(_, b)| b.as_slice()).collect();
    group.bench_function(
        BenchmarkId::new("multi_dex_parse", format!("{} files", dex_files.len())),
        |b| {
            b.iter(|| reseam_dex::MultiDexContainer::parse(&buffers, skip_verify_opts()).unwrap());
        },
    );
    let container = reseam_dex::MultiDexContainer::parse(&buffers, default_opts()).unwrap();
    group.bench_function(
        BenchmarkId::new("multi_dex_write", format!("{} files", dex_files.len())),
        |b| {
            b.iter_batched(
                || container.clone(),
                |mut c| c.write_all().unwrap(),
                criterion::BatchSize::LargeInput,
            );
        },
    );
    group.bench_function(BenchmarkId::new("lazy_parse_largest", largest_name), |b| {
        b.iter(|| {
            reseam_dex::parse(
                largest_buf,
                ParseOptions {
                    lazy: true,
                    skip_checksum: true,
                    skip_signature: true,
                    ..ParseOptions::default()
                },
            )
            .unwrap()
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_write,
    bench_round_trip,
    bench_search,
    bench_mutation,
    bench_multi_dex,
    bench_instagram
);
criterion_main!(benches);
