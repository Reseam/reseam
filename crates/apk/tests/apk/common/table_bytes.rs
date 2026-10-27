// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::tables::{config, make_test_arsc, package, res_type, simple, strings, test_package};
use reseam_apk::ResValue;
use reseam_apk::{ResourceTable, StringPool};

pub fn chunk_ranges(
    bytes: &[u8],
    range: std::ops::Range<usize>,
) -> Vec<(u16, std::ops::Range<usize>)> {
    let mut chunks = Vec::new();
    let mut at = range.start;
    while at + 8 <= range.end {
        let kind = u16::from_le_bytes(bytes[at..at + 2].try_into().expect("chunk kind"));
        let len =
            u32::from_le_bytes(bytes[at + 4..at + 8].try_into().expect("chunk size")) as usize;
        chunks.push((kind, at..at + len));
        at += len;
    }
    chunks
}

pub fn opaque_chunk(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(8u16.to_le_bytes());
    bytes.extend(((8 + payload.len()) as u32).to_le_bytes());
    bytes.extend(payload);
    bytes
}

pub fn extended_resource_table() -> Vec<u8> {
    let original = make_test_arsc().serialize().expect("fixture");
    let chunks = chunk_ranges(&original, 12..original.len());
    let global = &chunks.iter().find(|(kind, _)| *kind == 1).expect("pool").1;
    let package = &chunks
        .iter()
        .find(|(kind, _)| *kind == 0x200)
        .expect("package")
        .1;
    let children = chunk_ranges(&original, package.start + 288..package.end);
    let mut header = original[package.start..package.start + 288].to_vec();
    header.extend([0x13, 0x37, 0x42, 0x99]);
    header[2..4].copy_from_slice(&292u16.to_le_bytes());
    let mut body = Vec::new();
    for (index, (_, range)) in children.iter().enumerate() {
        if index == 0 {
            header[268..272].copy_from_slice(&((292 + body.len()) as u32).to_le_bytes());
        }
        if index == 1 {
            header[276..280].copy_from_slice(&((292 + body.len()) as u32).to_le_bytes());
        }
        body.extend_from_slice(&original[range.clone()]);
        if index == 1 {
            body.extend(opaque_chunk(
                0x206,
                &[1, 0, 0, 0, 1, 0, 1, 0x7f, 0, 0, 1, 0x7f],
            ));
        }
    }
    let package_len = header.len() + body.len();
    header[4..8].copy_from_slice(&(package_len as u32).to_le_bytes());
    let mut bytes = original[..12].to_vec();
    bytes[2..4].copy_from_slice(&16u16.to_le_bytes());
    bytes.extend([0x21, 0x43, 0x65, 0x87]);
    bytes.extend(opaque_chunk(0x7777, &[0x12, 0x34, 0x56, 0x78]));
    bytes.extend_from_slice(&original[global.clone()]);
    bytes.extend(header);
    bytes.extend(body);
    let len = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&len.to_le_bytes());
    bytes
}

pub fn alias_resource_table(header_size: u16, declared_count: u32) -> Vec<u8> {
    let original = extended_resource_table();
    let package = chunk_ranges(&original, 16..original.len())
        .into_iter()
        .find(|(kind, _)| *kind == 0x200)
        .expect("package")
        .1;
    let alias = chunk_ranges(&original, package.start + 292..package.end)
        .into_iter()
        .find(|(kind, _)| *kind == 0x206)
        .expect("alias")
        .1;
    let mut replacement = vec![0; usize::from(header_size)];
    replacement[..2].copy_from_slice(&0x0206u16.to_le_bytes());
    replacement[2..4].copy_from_slice(&header_size.to_le_bytes());
    replacement[8..12].copy_from_slice(&declared_count.to_le_bytes());
    for (staged, finalized) in [(0x7fff_0000u32, 0x7f01_0000u32), (0x7ffe_0000, 0x7f01_0001)] {
        replacement.extend(staged.to_le_bytes());
        replacement.extend(finalized.to_le_bytes());
    }
    let size = replacement.len() as u32;
    replacement[4..8].copy_from_slice(&size.to_le_bytes());
    let mut bytes = original[..alias.start].to_vec();
    bytes.extend(replacement);
    bytes.extend_from_slice(&original[alias.end..]);
    let size = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes[package.start + 4..package.start + 8]
        .copy_from_slice(&(size - package.start as u32).to_le_bytes());
    bytes
}

#[derive(Clone, Copy)]
pub enum ResourceIndex {
    Dense32,
    Dense16,
    Sparse,
}

pub fn indexed_resource_table(encoding: ResourceIndex, type_offset: u32, metadata: u8) -> Vec<u8> {
    let package = package(
        "com.example.index",
        &["string"],
        &["hello", "world"],
        [res_type(
            1,
            config(None, 0),
            [
                Some(simple(0, ResValue::STRING, 0)),
                None,
                Some(simple(1, ResValue::STRING, 1)),
            ],
        )],
    );
    let mut bytes = ResourceTable::new(strings(&["Hello", "World"]), vec![package])
        .serialize()
        .expect("table fixture");
    let package = chunk_ranges(&bytes, 12..bytes.len())
        .into_iter()
        .find(|(kind, _)| *kind == 0x0200)
        .expect("package")
        .1;
    bytes[package.start + 284..package.start + 288].copy_from_slice(&type_offset.to_le_bytes());
    let chunk = chunk_ranges(&bytes, package.start + 288..package.end)
        .into_iter()
        .find(|(kind, _)| *kind == 0x0201)
        .expect("type")
        .1;
    let header = u16::from_le_bytes(
        bytes[chunk.start + 2..chunk.start + 4]
            .try_into()
            .expect("header"),
    ) as usize;
    let payload = u32::from_le_bytes(
        bytes[chunk.start + 16..chunk.start + 20]
            .try_into()
            .expect("entry start"),
    ) as usize;
    let (flags, index) = match encoding {
        ResourceIndex::Dense32 => (0, vec![0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 16, 0, 0, 0]),
        ResourceIndex::Dense16 => (2, vec![0, 0, 0xff, 0xff, 4, 0, 0, 0]),
        ResourceIndex::Sparse => (1, vec![0, 0, 0, 0, 2, 0, 4, 0]),
    };
    let mut replacement = bytes[chunk.start..chunk.start + header].to_vec();
    replacement[9] = flags | metadata;
    let count = if matches!(encoding, ResourceIndex::Sparse) {
        2u32
    } else {
        3
    };
    replacement[12..16].copy_from_slice(&count.to_le_bytes());
    replacement[16..20].copy_from_slice(&((header + index.len()) as u32).to_le_bytes());
    replacement.extend(index);
    replacement.extend_from_slice(&bytes[chunk.start + payload..chunk.end]);
    let size = replacement.len() as u32;
    replacement[4..8].copy_from_slice(&size.to_le_bytes());
    let removed = chunk.len() - replacement.len();
    bytes.splice(chunk, replacement);
    let total = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&total.to_le_bytes());
    bytes[package.start + 4..package.start + 8]
        .copy_from_slice(&((package.len() - removed) as u32).to_le_bytes());
    bytes
}

pub fn resource_pool_fixture(strings: &[&str], encoding: reseam_apk::StringEncoding) -> Vec<u8> {
    ResourceTable::new(
        StringPool::new(strings.iter().map(ToString::to_string).collect(), encoding),
        vec![test_package()],
    )
    .serialize()
    .expect("resource pool fixture")
}

pub fn chunk(bytes: &[u8], range: std::ops::Range<usize>, kind: u16) -> std::ops::Range<usize> {
    chunk_ranges(bytes, range)
        .into_iter()
        .find(|(id, _)| *id == kind)
        .expect("fixture chunk")
        .1
}

pub fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixture word"))
}

pub fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixture word"))
}
