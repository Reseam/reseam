// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::header::read_header_at;
use crate::error::Result;
use crate::file::{ClassTable, DexBytes, DexFile, FileTable, IdTable, StringPool};
use crate::read::{read_u16, read_u32};
use crate::types::header::ParseOptions;
use tracing::{debug, instrument};

#[instrument(level = "debug", skip(buf), fields(buffer_len = buf.len(), classes = ?opts.classes))]
pub fn parse(buf: &[u8], opts: ParseOptions) -> Result<DexFile> {
    let raw = DexBytes::from_slice(buf);
    parse_single_with_raw(raw, opts, None)
}

#[instrument(level = "debug", skip(buf), fields(buffer_len = buf.len(), classes = ?opts.classes))]
pub fn parse_owned(buf: Vec<u8>, opts: ParseOptions) -> Result<DexFile> {
    let raw = DexBytes::from_vec(buf);
    parse_single_with_raw(raw, opts, None)
}

pub fn parse_bytes(raw: DexBytes, opts: ParseOptions) -> Result<DexFile> {
    let dex = parse_single_with_raw(raw, opts, None)?;
    dex.release_pages();
    Ok(dex)
}

/// Parses a v41 container buffer into its constituent logical DEX files.
///
/// For non-container buffers (v40 and earlier), returns a single-element vec.
#[instrument(level = "debug", skip(buf), fields(buffer_len = buf.len(), classes = ?opts.classes))]
pub fn parse_container(buf: &[u8], opts: ParseOptions) -> Result<Vec<DexFile>> {
    parse_container_with_bytes(DexBytes::from_slice(buf), opts)
}

/// Parses every logical DEX in shared storage without copying its bytes.
/// Earlier DEX versions produce one file. Members share the backing storage,
/// which must remain unchanged while they live; decoding follows `opts`.
/// A malformed member fails the entire container.
pub fn parse_container_with_bytes(raw: DexBytes, opts: ParseOptions) -> Result<Vec<DexFile>> {
    let buf = raw.as_bytes();
    if buf.len() < 8 {
        let dex = parse_single_with_raw(raw, opts, None)?;
        return Ok(vec![dex]);
    }

    let mut magic = [0u8; 8];
    magic.copy_from_slice(&buf[..8]);
    let version = crate::types::header::DexVersion::from_magic(magic);

    let is_container =
        version.is_some_and(super::super::types::header::DexVersion::is_container_format);
    if !is_container {
        let dex = parse_single_with_raw(raw, opts, None)?;
        return Ok(vec![dex]);
    }

    let first = read_header_at(buf, 0, opts)?;
    let end = first.container_size as usize;
    let mut dex_files = Vec::new();
    let mut offset = 0;
    while offset < end {
        let dex = parse_single_with_raw(raw.clone(), opts, Some(offset))?;
        if !dex.header.version.is_container_format() || dex.header.container_size as usize != end {
            return Err(crate::error::invalid(
                "dex container",
                "inconsistent member header",
            ));
        }
        offset += dex.header.file_size as usize;
        dex_files.push(dex);
    }
    debug!(dex_count = dex_files.len(), "parsed DEX container");
    raw.release_pages();
    drop(raw);
    Ok(dex_files)
}

fn parse_single_with_raw(
    raw: DexBytes,
    opts: ParseOptions,
    header_off: Option<usize>,
) -> Result<DexFile> {
    let buf = raw.as_bytes();
    let header = read_header_at(buf, header_off.unwrap_or(0), opts)?;
    crate::error::require_len(
        raw.as_bytes(),
        0,
        header.container_size as usize,
        "DEX storage",
    )?;
    let raw = raw
        .bounded(header.container_size as usize)
        .expect("container size was checked");
    let buf = raw.as_bytes();

    let mut dex = DexFile::new(header.clone());
    dex.parse_options = opts;
    dex.strings = StringPool::from_raw(
        raw.clone(),
        header.string_ids_off,
        header.string_ids_size,
        opts,
    )?;
    dex.types = IdTable::from_raw(raw.clone(), header.type_ids_off, header.type_ids_size)?;
    dex.prototypes = IdTable::from_raw(raw.clone(), header.proto_ids_off, header.proto_ids_size)?;
    dex.fields = IdTable::from_raw(raw.clone(), header.field_ids_off, header.field_ids_size)?;
    dex.methods = IdTable::from_raw(raw.clone(), header.method_ids_off, header.method_ids_size)?;
    dex.classes = ClassTable::from_raw(raw.clone(), header.class_defs_off, header.class_defs_size)?;

    dex.raw = Some(raw.clone());
    let map_off = header.map_off as usize;
    let map_size = read_u32(buf, map_off)? as usize;

    crate::error::require_array(buf, map_off + 4, map_size, 12, "map list")?;

    let mut call_site_off: Option<(u32, u32)> = None;
    let mut method_handle_off: Option<(u32, u32)> = None;
    let mut hidden_api_off: Option<(u32, u32)> = None;

    for i in 0..map_size {
        let entry = map_off + 4 + i * 12;
        let type_code = read_u16(buf, entry)?;
        let size = read_u32(buf, entry + 4)?;
        let offset = read_u32(buf, entry + 8)?;

        match type_code {
            0x0007 => call_site_off = Some((offset, size)),
            0x0008 => method_handle_off = Some((offset, size)),
            0xF000 => hidden_api_off = Some((offset, size)),
            _ => {}
        }
    }

    if header.version.supports_call_sites() {
        if let Some((offset, count)) = method_handle_off {
            dex.method_handles = FileTable::from_raw(raw.clone(), offset, count, opts)?;
        }
        if let Some((offset, count)) = call_site_off {
            dex.call_sites = FileTable::from_raw(raw.clone(), offset, count, opts)?;
        }
    }

    crate::references::validate_pools(&dex)?;

    if header.version.supports_hidden_api()
        && let Some((off, _)) = hidden_api_off
    {
        dex.hidden_api = Some(super::hidden_api::read_hidden_api(
            raw.clone(),
            off as usize,
            &dex,
            opts,
        )?);
    }

    if !(opts.classes == crate::types::header::Loading::Deferred) {
        dex.classes.materialize_all(opts)?;
        for class in dex.classes.iter_resident() {
            crate::references::validate_class(&dex, class)?;
        }
    }

    debug!(
        version = ?dex.header.version,
        string_count = dex.strings.len(),
        type_count = dex.types.len(),
        method_count = dex.methods.len(),
        class_count = dex.classes.len(),
        "parsed DEX file"
    );
    dex.raw = Some(raw);
    Ok(dex)
}
