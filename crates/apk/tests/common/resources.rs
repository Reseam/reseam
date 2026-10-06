// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::axml::{self, AxmlAttribute, AxmlDocument, AxmlEvent};
use reseam_apk::resources::{
    EntryValue, MapEntry, ResEntry, ResPackage, ResType, ResourceTable, TypeSpec,
};
use reseam_apk::{ResValue, ResourceScope, StringPool};
use reseam_dex::file::DexBytes;

pub fn make_test_axml(encoding: reseam_apk::StringEncoding) -> AxmlDocument {
    let strings = [
        "http://schemas.android.com/apk/res/android",
        "android",
        "manifest",
        "package",
        "versionCode",
        "versionName",
        "com.example.test",
        "1.0.0",
    ];
    let elements = vec![
        AxmlEvent::StartNamespace {
            metadata: axml::NodeMetadata::default(),
            prefix: Some(1),
            uri: 0,
        },
        AxmlEvent::StartElement {
            metadata: axml::NodeMetadata::default(),
            namespace: None,
            name: 2,
            attributes: vec![
                AxmlAttribute::new(None, 3, ResValue::string(6)),
                AxmlAttribute::new(Some(0), 4, ResValue::int(1)),
                AxmlAttribute::new(Some(0), 5, ResValue::string(7)),
            ],
        },
        AxmlEvent::EndElement {
            metadata: axml::NodeMetadata::default(),
            namespace: None,
            name: 2,
        },
        AxmlEvent::EndNamespace {
            metadata: axml::NodeMetadata::default(),
            prefix: Some(1),
            uri: 0,
        },
    ];
    AxmlDocument::from_parts(
        StringPool::new(strings.iter().map(ToString::to_string).collect(), encoding),
        vec![0, 0, 0, 0, 0x0101_021b, 0x0101_021c],
        elements,
    )
    .expect("valid XML fixture")
}

pub fn strings(values: &[&str]) -> StringPool {
    StringPool::new(
        values.iter().map(ToString::to_string).collect(),
        reseam_apk::StringEncoding::Utf8,
    )
}

pub fn simple(key: u32, kind: u8, data: u32) -> ResEntry {
    ResEntry {
        flags: 0,
        key,
        value: EntryValue::Simple(ResValue::new(kind, data)),
    }
}

pub fn make_test_arsc() -> ResourceTable {
    let mut pkg = ResPackage::new(
        0x7F,
        "com.example.test",
        strings(&["string"]),
        strings(&["hello", "world"]),
    );
    pkg.add_type_spec(TypeSpec::new(1, vec![0, 0]));
    let mut t = ResType::new(1, vec![0; 48]);
    t.push(Some(simple(0, ResValue::STRING, 0)))
        .expect("resource fixture entry");
    t.push(Some(simple(1, ResValue::STRING, 1)))
        .expect("resource fixture entry");
    pkg.add_type(t);
    ResourceTable::new(strings(&["Hello", "World", "app_name"]), vec![pkg])
}

pub fn make_attr_arsc() -> ResourceTable {
    const ATTR_TYPE: u32 = 0x0100_0000;
    let mut pkg = ResPackage::new(
        0x7F,
        "com.example.attrs",
        strings(&["attr", "id"]),
        strings(&["mode", "sides", "compact", "wide", "top", "bottom"]),
    );
    let bag = |key: u32, format: u32, symbols: [(u32, u32); 2]| {
        Some(ResEntry {
            flags: 0x0001,
            key,
            value: EntryValue::Complex {
                parent: 0,
                entries: std::iter::once(MapEntry {
                    name: ATTR_TYPE,
                    value: ResValue::int(format as i32),
                })
                .chain(symbols.map(|(name, value)| MapEntry {
                    name,
                    value: ResValue::int(value as i32),
                }))
                .collect(),
            },
        })
    };
    pkg.add_type_spec(TypeSpec::new(1, vec![0, 0]));
    let mut attrs = ResType::new(1, vec![0; 48]);
    attrs
        .push(bag(0, 1 << 16, [(0x7F02_0000, 1), (0x7F02_0001, 2)]))
        .expect("resource fixture entry");
    attrs
        .push(bag(1, 1 << 17, [(0x7F02_0002, 1), (0x7F02_0003, 2)]))
        .expect("resource fixture entry");
    pkg.add_type(attrs);
    pkg.add_type_spec(TypeSpec::new(2, vec![0; 4]));
    let mut ids = ResType::new(2, vec![0; 48]);
    for key in 2..6 {
        ids.push(Some(simple(key, ResValue::INT_BOOLEAN, 0)))
            .expect("resource fixture entry");
    }
    pkg.add_type(ids);
    ResourceTable::new(strings(&[]), vec![pkg])
}

pub fn table_with_configs(
    type_name: &str,
    entry_name: &str,
    configs: &[(Vec<u8>, EntryValue)],
    global: &[&str],
) -> ResourceTable {
    let mut pkg = ResPackage::new(
        0x7F,
        "com.example",
        strings(&[type_name]),
        strings(&[entry_name]),
    );
    pkg.add_type_spec(TypeSpec::new(1, vec![0]));
    for (config, value) in configs {
        let mut res_type = ResType::new(1, config.clone());
        res_type
            .push(Some(ResEntry {
                flags: 0,
                key: 0,
                value: value.clone(),
            }))
            .expect("resource fixture entry");
        pkg.add_type(res_type);
    }
    ResourceTable::new(strings(global), vec![pkg])
}

pub fn density_config(density: u16) -> Vec<u8> {
    let mut config = vec![0u8; 48];
    config[14..16].copy_from_slice(&density.to_le_bytes());
    config
}

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

pub fn extended_axml() -> Vec<u8> {
    let mut doc = axml::build_document(r#"<View xmlns:android="http://schemas.android.com/apk/res/android" android:id="@0x7f010000" class="CustomView" style="@0x7f020000" android:enabled="true"/>"#, None).expect("fixture XML");
    let text = doc.intern_string("content");
    let original = doc.serialize().expect("fixture bytes");
    let root = doc.root().expect("root");
    let roles = ["id", "class", "style"].map(|name| {
        doc.attributes(root)
            .iter()
            .position(|attr| doc.string(attr.name).as_deref() == Some(name))
            .expect("special attribute") as u16
            + 1
    });
    let mut bytes = original[..8].to_vec();
    bytes[2..4].copy_from_slice(&12u16.to_le_bytes());
    bytes.extend([0x10, 0x20, 0x30, 0x40]);
    bytes.extend(opaque_chunk(0x1777, &[0x55, 0x66, 0x77, 0x88]));
    for (kind, range) in chunk_ranges(&original, 8..original.len()) {
        if !(0x100..=0x103).contains(&kind) {
            bytes.extend_from_slice(&original[range]);
            continue;
        }
        let node = &original[range];
        let mut encoded = node[..16].to_vec();
        encoded[2..4].copy_from_slice(&20u16.to_le_bytes());
        encoded[8..12].copy_from_slice(&37u32.to_le_bytes());
        encoded[12..16].copy_from_slice(&text.to_le_bytes());
        encoded.extend([0xde, 0xad, 0xbe, 0xef]);
        if kind == 0x102 {
            let count = doc.attributes(root).len();
            let mut extension = node[16..36].to_vec();
            extension[8..10].copy_from_slice(&24u16.to_le_bytes());
            extension[10..12].copy_from_slice(&24u16.to_le_bytes());
            for (role, offset) in [14, 16, 18].into_iter().enumerate() {
                extension[offset..offset + 2].copy_from_slice(&roles[role].to_le_bytes());
            }
            encoded.extend(extension);
            encoded.extend([0x11, 0x22, 0x33, 0x44]);
            for attr in node[36..36 + count * 20].as_chunks::<20>().0 {
                encoded.extend_from_slice(attr);
                encoded.extend([0xab, 0xcd, 0xef, 0x12]);
            }
            encoded.extend([0x99, 0x88, 0x77, 0x66]);
        } else {
            encoded.extend_from_slice(&node[16..]);
        }
        let len = encoded.len() as u32;
        encoded[4..8].copy_from_slice(&len.to_le_bytes());
        bytes.extend(encoded);
        if kind == 0x102 {
            let mut cdata = vec![4, 1, 16, 0, 28, 0, 0, 0];
            cdata.extend(38u32.to_le_bytes());
            cdata.extend(u32::MAX.to_le_bytes());
            cdata.extend(text.to_le_bytes());
            cdata.extend([8, 0, 0, 3]);
            cdata.extend(text.to_le_bytes());
            bytes.extend(cdata);
        }
    }
    let len = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&len.to_le_bytes());
    bytes
}

pub fn attribute_table(definitions: &[(&str, u32)]) -> ResourceTable {
    let mut package = ResPackage::new(
        0x7f,
        "com.example",
        strings(&["attr"]),
        strings(
            &definitions
                .iter()
                .map(|&(name, _)| name)
                .collect::<Vec<_>>(),
        ),
    );
    package.add_type_spec(TypeSpec::new(1, vec![0; definitions.len()]));
    let mut attrs = ResType::new(1, vec![0; 48]);
    for (key, &(_, formats)) in definitions.iter().enumerate() {
        attrs
            .push(Some(ResEntry {
                flags: 0,
                key: key as u32,
                value: EntryValue::Complex {
                    parent: 0,
                    entries: vec![MapEntry {
                        name: 0x0100_0000,
                        value: ResValue::int(formats as i32),
                    }],
                },
            }))
            .expect("resource fixture entry");
    }
    package.add_type(attrs);
    ResourceTable::new(strings(&[]), vec![package])
}

#[derive(Clone, Copy)]
pub enum ResourceIndex {
    Dense32,
    Dense16,
    Sparse,
}

pub fn indexed_resource_table(encoding: ResourceIndex, type_offset: u32, metadata: u8) -> Vec<u8> {
    let mut package = ResPackage::new(
        0x7f,
        "com.example.index",
        strings(&["string"]),
        strings(&["hello", "world"]),
    );
    let mut entries = ResType::new(1, vec![0; 48]);
    entries
        .push(Some(simple(0, ResValue::STRING, 0)))
        .expect("entry");
    entries.push(None).expect("absent entry");
    entries
        .push(Some(simple(1, ResValue::STRING, 1)))
        .expect("entry");
    package.add_type_spec(TypeSpec::new(1, vec![0; 3]));
    package.add_type(entries);
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
    let source = make_test_arsc();
    ResourceTable::new(
        StringPool::new(strings.iter().map(ToString::to_string).collect(), encoding),
        source.packages().to_vec(),
    )
    .serialize()
    .expect("resource pool fixture")
}

pub fn view(attributes: &str, table: Option<&mut ResourceTable>) -> AxmlDocument {
    let text = format!(
        r#"<View xmlns:android="http://schemas.android.com/apk/res/android" xmlns:app="http://schemas.android.com/apk/res-auto" {attributes}/>"#
    );
    axml::build_document(&text, table.map(ResourceScope::from).as_mut()).expect("fixture XML")
}

pub fn attribute<'a>(doc: &'a AxmlDocument, element: usize, name: &str) -> &'a AxmlAttribute {
    doc.attribute_named(element, name)
        .expect("fixture attribute")
}

pub fn root_value(doc: &AxmlDocument, name: &str) -> ResValue {
    attribute(doc, doc.root().expect("fixture root"), name).value
}

pub fn round_trip(table: &ResourceTable) -> ResourceTable {
    ResourceTable::parse(DexBytes::from_vec(
        table.serialize().expect("fixture table"),
    ))
    .expect("fixture table")
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
