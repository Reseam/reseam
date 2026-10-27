// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Case, Readback, text_values};
use crate::common::table_bytes::*;
use crate::common::tables::*;
use reseam_apk::resources::{ResPackage, ResourceTable};
use reseam_apk::{ResValue, StringEncoding};
use std::borrow::Cow;

pub(super) fn strings_case() -> reseam_apk::Result<Case> {
    let table = ResourceTable::new(
        test_strings(),
        vec![
            ResPackage::new(0x7e, "empty.pkg", strings(&[]), strings(&[])),
            test_package(),
        ],
    );
    Ok(Case {
        input: table.serialize()?,
        edit: Box::new(|table| {
            table.set_string(0, "Modified".into())?;
            let reference = &table.find_entries_by_string(1)?[0];
            table.replace_entry_string(reference.res_id, 0)
        }),
        read: Box::new(|table| {
            Ok(Readback {
                texts: text_values(table, &["hello", "world"])?,
                numbers: vec![u32::from(
                    table.find_resource_id("string", "absent")?.is_none(),
                )],
                ..Default::default()
            })
        }),
        expected: Readback {
            texts: vec!["Modified".into(); 2],
            numbers: vec![1],
            ..Default::default()
        },
    })
}

pub(super) fn index_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for encoding in [
        ResourceIndex::Dense32,
        ResourceIndex::Dense16,
        ResourceIndex::Sparse,
    ] {
        for (offset, metadata) in [(0, 0), (7, 0), (0, 0x80), (7, 0x80)] {
            let hello = 0x7f00_0000 | (offset + 1) << 16;
            cases.push(Case {
                input: indexed_resource_table(encoding, offset, metadata),
                edit: Box::new(move |table| {
                    let replacement = table.add_global_string("Changed");
                    table.replace_entry_string(hello | 2, replacement)?;
                    table.set_string_value_by_id(hello, "Greeting")?;
                    let added = ResValue::string(table.add_global_string("Added"));
                    table.add_resource("string", "added", added)?;
                    Ok(())
                }),
                read: Box::new(move |table| {
                    let bytes = table.serialize()?;
                    let pkg = chunk(&bytes, 12..bytes.len(), 0x200);
                    let t = chunk(&bytes, pkg.start + 288..pkg.end, 0x201);
                    let mut texts = text_values(table, &["hello", "world", "added"])?;
                    for res_id in [hello, 0x0104_0000] {
                        texts.push(
                            table
                                .string_value_by_id(res_id)?
                                .map_or("absent".into(), Cow::into_owned),
                        );
                    }
                    Ok(Readback {
                        texts,
                        numbers: ["hello", "world", "added"]
                            .iter()
                            .map(|name| {
                                Ok(table.find_resource_id("string", name)?.expect("fixture ID"))
                            })
                            .collect::<reseam_apk::Result<_>>()?,
                        bytes: vec![
                            vec![bytes[t.start + 9] & !3],
                            vec![u8::from(table.packages()[0].types()[0].entry(1)?.is_none())],
                        ],
                        ..Default::default()
                    })
                }),
                expected: Readback {
                    texts: ["Greeting", "Changed", "Added", "Greeting", "absent"]
                        .map(String::from)
                        .to_vec(),
                    numbers: vec![hello, hello | 2, hello | 3],
                    bytes: vec![vec![metadata], vec![1]],
                    ..Default::default()
                },
            });
        }
    }
    cases
}

pub(super) fn extended_case() -> Case {
    Case {
        input: extended_resource_table(),
        edit: Box::new(|table| {
            let added = ResValue::string(table.add_global_string("Added"));
            table.add_resource("string", "new", added)?;
            table.set_package_name("com.changed")
        }),
        read: Box::new(|table| {
            let bytes = table.serialize()?;
            let chunks = chunk_ranges(&bytes, 16..bytes.len());
            let pkg = &chunks[2].1;
            let children = chunk_ranges(&bytes, pkg.start + 292..pkg.end);
            let mut texts = text_values(table, &["hello", "new"])?;
            texts.push(table.packages()[0].name());
            Ok(Readback {
                texts,
                numbers: chunks
                    .iter()
                    .chain(&children)
                    .map(|(kind, _)| u32::from(*kind))
                    .collect(),
                bytes: vec![
                    bytes[12..16].to_vec(),
                    bytes[pkg.start + 288..pkg.start + 292].to_vec(),
                    bytes[children[2].1.clone()].to_vec(),
                ],
                ..Default::default()
            })
        }),
        expected: Readback {
            texts: ["Hello", "Added", "com.changed"].map(String::from).to_vec(),
            numbers: vec![0x7777, 1, 0x200, 1, 1, 0x206, 0x202, 0x201],
            bytes: vec![
                vec![0x21, 0x43, 0x65, 0x87],
                vec![0x13, 0x37, 0x42, 0x99],
                opaque_chunk(0x206, &[1, 0, 0, 0, 1, 0, 1, 0x7f, 0, 0, 1, 0x7f]),
            ],
            ..Default::default()
        },
    }
}

pub(super) fn pool_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for empty in [false, true] {
        let mut bytes = resource_pool_fixture(
            if empty { &[] } else { &["X", "World"] },
            if empty {
                StringEncoding::Utf8
            } else {
                StringEncoding::Utf16
            },
        );
        let pool = chunk(&bytes, 12..bytes.len(), 1).start;
        if empty {
            bytes[pool + 20..pool + 24].fill(0);
        } else {
            let payload = pool + u32_at(&bytes, pool + 20) as usize;
            bytes[payload + 2..payload + 4].copy_from_slice(&0xd800u16.to_le_bytes());
        }
        cases.push(Case {
            input: bytes,
            edit: Box::new(move |table| {
                table.add_global_string(if empty { "First" } else { "Added" });
                Ok(())
            }),
            read: Box::new(move |table| {
                let mut result = Readback {
                    texts: vec![table.get_string(0)?.expect("fixture string").into_owned()],
                    ..Default::default()
                };
                if !empty {
                    let bytes = table.serialize()?;
                    let pool = chunk(&bytes, 12..bytes.len(), 1).start;
                    let payload = pool + u32_at(&bytes, pool + 20) as usize;
                    result.bytes.push(bytes[payload..payload + 6].to_vec());
                }
                Ok(result)
            }),
            expected: Readback {
                texts: vec![if empty { "First" } else { "�" }.into()],
                bytes: if empty {
                    vec![]
                } else {
                    vec![vec![1, 0, 0, 0xd8, 0, 0]]
                },
                ..Default::default()
            },
        });
    }
    cases
}
