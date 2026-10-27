// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use reseam_dex::{self as dex, DexFile, Loading, ParseOptions};

/// Decodes every method body of each DEX (or each `classes*.dex` inside an
/// APK), re-encodes it, and fails unless the code units come back unchanged.
pub(super) fn round_trip(paths: &[PathBuf]) -> Result<()> {
    let mut out = std::io::stdout().lock();
    let mut failures = 0;
    let (mut instructions, mut decode_time) = (0, Duration::ZERO);
    for path in paths {
        for (name, bytes) in dex_files(path)? {
            let report = check(&bytes).with_context(|| format!("{}!{name}", path.display()))?;
            writeln!(
                out,
                "{}!{name}: {} methods, {} instructions, decoded in {:.1?}, {} mismatches",
                path.display(),
                report.methods,
                report.instructions,
                report.decode_time,
                report.mismatches.len(),
            )?;
            for method in report.mismatches.iter().take(5) {
                writeln!(out, "  code item at {method:#x} does not round-trip")?;
            }
            failures += report.mismatches.len();
            instructions += report.instructions;
            decode_time += report.decode_time;
        }
    }
    writeln!(
        out,
        "{instructions} instructions decoded in {decode_time:.1?}"
    )?;
    if failures != 0 {
        bail!("{failures} code items do not round-trip");
    }
    Ok(())
}

struct Report {
    methods: usize,
    instructions: usize,
    decode_time: Duration,
    mismatches: Vec<u32>,
}

fn check(bytes: &[u8]) -> Result<Report> {
    let file = dex::parse(
        bytes,
        ParseOptions {
            classes: Loading::Deferred,
            ..ParseOptions::default()
        },
    )?;
    let bodies = code_items(&file)?;
    let buf = file.raw_buffer().context("parsed DEX keeps its bytes")?;
    let started = Instant::now();
    let decoded = bodies
        .iter()
        .map(|&(start, units)| dex::decode_instructions(buf, start, units))
        .collect::<dex::Result<Vec<_>>>()?;
    let decode_time = started.elapsed();
    let mut report = Report {
        methods: bodies.len(),
        instructions: decoded.iter().map(Vec::len).sum(),
        decode_time,
        mismatches: Vec::new(),
    };
    for (&(start, units), instructions) in bodies.iter().zip(&decoded) {
        let original: Vec<u16> = buf[start..start + units * 2]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&unit| u16::from_le_bytes(unit))
            .collect();
        if dex::encode_instructions(instructions)? != original {
            report.mismatches.push(start as u32 - 16);
        }
    }
    Ok(report)
}

/// The instruction stream of every method body: its byte offset and code units.
fn code_items(file: &DexFile) -> Result<Vec<(usize, usize)>> {
    let buf = file.raw_buffer().context("parsed DEX keeps its bytes")?;
    let mut bodies = Vec::new();
    for class in 0..file.classes().len() {
        let Some(skeleton) = file.class_skeleton(class)? else {
            continue;
        };
        for method in skeleton
            .direct_methods
            .iter()
            .chain(&skeleton.virtual_methods)
        {
            if method.code_off == 0 {
                continue;
            }
            let at = method.code_off as usize;
            let units = u32::from_le_bytes(buf[at + 12..at + 16].try_into()?) as usize;
            bodies.push((at + 16, units));
        }
    }
    Ok(bodies)
}

fn dex_files(path: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    if path.extension().is_some_and(|ext| ext == "dex") {
        return Ok(vec![(String::new(), std::fs::read(path)?)]);
    }
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        let entry_path = Path::new(&name);
        if entry_path.parent() == Some(Path::new(""))
            && entry_path.extension().is_some_and(|ext| ext == "dex")
            && name.starts_with("classes")
        {
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut bytes)?;
            files.push((name, bytes));
        }
    }
    Ok(files)
}
