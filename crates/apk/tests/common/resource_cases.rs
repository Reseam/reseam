// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::fixtures::*;
use reseam_apk::resources::{AttrFormats, EntryValue, MapEntry, ResPackage, ResourceTable};
use reseam_apk::{ResValue, ResourceScope, StringEncoding};

type Read = Box<dyn Fn(&ResourceTable) -> reseam_apk::Result<Readback>>;
type Edit = Box<dyn Fn(&mut ResourceTable) -> reseam_apk::Result<()>>;

#[derive(Debug, Default, PartialEq)]
pub struct Readback {
    pub texts: Vec<String>,
    pub numbers: Vec<u32>,
    pub values: Vec<ResValue>,
    pub bytes: Vec<Vec<u8>>,
}

pub struct Case {
    pub input: Vec<u8>,
    pub edit: Edit,
    pub read: Read,
    pub expected: Readback,
}

fn text_values(table: &ResourceTable, names: &[&str]) -> reseam_apk::Result<Vec<String>> {
    names
        .iter()
        .map(|name| {
            Ok(table
                .string_value(name)?
                .expect("fixture string")
                .into_owned())
        })
        .collect()
}

pub fn cases() -> reseam_apk::Result<Vec<Case>> {
    let mut cases = vec![
        strings_case()?,
        extended_case(),
        style_case()?,
        array_case()?,
    ];
    cases.extend(index_cases());
    cases.extend(file_cases()?);
    cases.extend(pool_cases());
    Ok(cases)
}

fn strings_case() -> reseam_apk::Result<Case> {
    let fixture = make_test_arsc();
    let table = ResourceTable::new(
        fixture.global_strings().clone(),
        vec![
            ResPackage::new(0x7e, "empty.pkg", strings(&[]), strings(&[])),
            fixture.packages()[0].clone(),
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

fn index_cases() -> Vec<Case> {
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
                    table.add_string_resource("added", "Added")?;
                    Ok(())
                }),
                read: Box::new(|table| {
                    let bytes = table.serialize()?;
                    let pkg = chunk(&bytes, 12..bytes.len(), 0x200);
                    let t = chunk(&bytes, pkg.start + 288..pkg.end, 0x201);
                    Ok(Readback {
                        texts: text_values(table, &["hello", "world", "added"])?,
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
                    texts: ["Hello", "Changed", "Added"].map(String::from).to_vec(),
                    numbers: vec![hello, hello | 2, hello | 3],
                    bytes: vec![vec![metadata], vec![1]],
                    ..Default::default()
                },
            });
        }
    }
    cases
}

fn extended_case() -> Case {
    Case {
        input: extended_resource_table(),
        edit: Box::new(|table| {
            table.add_string_resource("new", "Added")?;
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

fn style_case() -> reseam_apk::Result<Case> {
    let mut night = vec![0; 48];
    night[13] = 0x20;
    let existing = EntryValue::Complex {
        parent: 0,
        entries: vec![MapEntry {
            name: 0x0101_0098,
            value: ResValue::new(ResValue::INT_COLOR_ARGB8, 0xff00_0000),
        }],
    };
    Ok(Case {
        input: table_with_configs(
            "style",
            "AppTheme",
            &[(vec![0; 48], existing.clone()), (night, existing)],
            &[],
        )
        .serialize()?,
        edit: Box::new(|table| {
            ResourceScope::from(table).set_style_items(
                "AppTheme",
                None,
                &[
                    ("android:windowBackground".into(), "#ffffffff".into()),
                    ("android:textColor".into(), "#ff102030".into()),
                ],
            )?;
            Ok(())
        }),
        read: Box::new(|table| {
            let mut result = Readback::default();
            for config in table.packages()[0].types() {
                let EntryValue::Complex { entries, .. } =
                    config.entry(0)?.expect("fixture style").value
                else {
                    panic!("bag");
                };
                result.numbers.extend(entries.iter().map(|e| e.name));
                result.values.extend(entries.iter().map(|e| e.value));
            }
            Ok(result)
        }),
        expected: Readback {
            numbers: vec![0x0101_0054, 0x0101_0098, 0x0101_0054, 0x0101_0098],
            values: [
                ResValue::new(ResValue::INT_COLOR_ARGB8, 0xffff_ffff),
                ResValue::new(ResValue::INT_COLOR_ARGB8, 0xff10_2030),
            ]
            .repeat(2),
            ..Default::default()
        },
    })
}

fn array_case() -> reseam_apk::Result<Case> {
    let literals = ["true", "17", "@null", "#123456"]
        .map(String::from)
        .to_vec();
    let mut values: Vec<_> = (0..4).map(ResValue::string).collect();
    values.extend([
        ResValue::boolean(true),
        ResValue::int(-7),
        ResValue::hex(17),
        ResValue::reference(0x7f01_0000),
        ResValue::new(ResValue::INT_COLOR_RGB4, 0xff11_2233),
        ResValue::new(0x08, 0x7f01_0000),
    ]);
    Ok(Case {
        input: table_with_configs(
            "array",
            "values",
            &[(
                vec![0; 48],
                EntryValue::Complex {
                    parent: 0,
                    entries: Vec::new(),
                },
            )],
            &[],
        )
        .serialize()?,
        edit: Box::new(|table| {
            table.set_string_array(
                "values",
                &["true", "17", "@null", "#123456"].map(String::from),
            )?;
            let mut values = table.array_values("values")?;
            values.extend([
                ResValue::boolean(true),
                ResValue::int(-7),
                ResValue::hex(17),
                ResValue::reference(0x7f01_0000),
                ResValue::new(ResValue::INT_COLOR_RGB4, 0xff11_2233),
                ResValue::new(0x08, 0x7f01_0000),
            ]);
            table.set_array_values("values", &values)?;
            Ok(())
        }),
        read: Box::new(|table| {
            Ok(Readback {
                texts: (0..4)
                    .map(|i| Ok(table.get_string(i)?.expect("fixture literal").into_owned()))
                    .collect::<reseam_apk::Result<_>>()?,
                values: table.array_values("values")?,
                ..Default::default()
            })
        }),
        expected: Readback {
            texts: literals,
            values,
            ..Default::default()
        },
    })
}

fn file_cases() -> reseam_apk::Result<Vec<Case>> {
    let mut cases = Vec::new();
    for initial in [vec![0; 48], density_config(480)] {
        cases.push(Case {
            input: table_with_configs(
                "mipmap",
                "launcher",
                &[(initial, EntryValue::Simple(ResValue::string(0)))],
                &["res/launcher.png"],
            )
            .serialize()?,
            edit: Box::new(|table| {
                for (qualifier, path) in [
                    ("", "res/default.png"),
                    ("hdpi", "res/hdpi.png"),
                    ("xxhdpi", "res/xxhdpi.png"),
                    ("anydpi-v26", "res/adaptive.xml"),
                ] {
                    table.add_file_resource("mipmap", "reseam_icon", path, qualifier)?;
                }
                Ok(())
            }),
            read: Box::new(|table| {
                let mut result = Readback {
                    texts: table.file_paths("mipmap", "launcher")?,
                    ..Default::default()
                };
                let id = table
                    .find_resource_id("mipmap", "reseam_icon")?
                    .expect("fixture ID");
                for (density, sdk) in [(0, 0), (240, 0), (480, 0), (0xfffe, 26)] {
                    let config = table.packages()[0]
                        .types()
                        .iter()
                        .find(|t| {
                            u16_at(t.config(), 14) == density && u16_at(t.config(), 24) == sdk
                        })
                        .expect("fixture configuration");
                    let EntryValue::Simple(value) = config
                        .entry((id & 0xffff) as usize)?
                        .expect("fixture file")
                        .value
                    else {
                        panic!("simple entry");
                    };
                    result.texts.push(
                        table
                            .get_string(value.data)?
                            .expect("fixture path")
                            .into_owned(),
                    );
                }
                result
                    .numbers
                    .push(table.file_paths("mipmap", "reseam_icon")?.len() as u32);
                Ok(result)
            }),
            expected: Readback {
                texts: [
                    "res/launcher.png",
                    "res/default.png",
                    "res/hdpi.png",
                    "res/xxhdpi.png",
                    "res/adaptive.xml",
                ]
                .map(String::from)
                .to_vec(),
                numbers: vec![4],
                ..Default::default()
            },
        });
    }
    Ok(cases)
}

fn pool_cases() -> Vec<Case> {
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

pub struct XmlCase {
    pub doc: reseam_apk::axml::AxmlDocument,
    pub values: Vec<(&'static str, ResValue)>,
    pub kinds: Vec<(&'static str, u8)>,
}

pub fn xml_cases() -> reseam_apk::Result<Vec<XmlCase>> {
    let mut table = make_test_arsc();
    let title = table
        .add_resource("attr", "titleText", ResValue::new(0, 0))?
        .expect("fixture ID");
    let doc = view(
        r#"android:text="@string/hello" android:id="@+id/title" android:theme="?attr/titleText" android:textColor="?android:attr/textColor" android:background="@android:color/white" android:padding="16dp" android:alpha="0.5""#,
        Some(&mut table),
    );
    Ok(vec![
        XmlCase {
            doc,
            values: vec![
                (
                    "text",
                    ResValue::reference(table.find_resource_id("string", "hello")?.expect("ID")),
                ),
                (
                    "id",
                    ResValue::reference(table.find_resource_id("id", "title")?.expect("ID")),
                ),
                ("theme", ResValue::attribute(title)),
                (
                    "textColor",
                    ResValue::attribute(
                        reseam_apk::axml::android_attr_res_id("textColor").expect("ID"),
                    ),
                ),
                (
                    "background",
                    ResValue::reference(
                        reseam_apk::axml::android_res_id("color", "white").expect("ID"),
                    ),
                ),
            ],
            kinds: vec![("padding", ResValue::DIMENSION), ("alpha", ResValue::FLOAT)],
        },
        XmlCase {
            doc: view(
                r#"android:scaleType="center" android:gravity="center_vertical | end" android:contentDescription="center" app:mode="wide" app:sides="top|bottom""#,
                Some(&mut make_attr_arsc()),
            ),
            values: vec![
                ("scaleType", ResValue::int(5)),
                ("gravity", ResValue::hex(0x0080_0015)),
                ("mode", ResValue::int(2)),
                ("sides", ResValue::hex(3)),
            ],
            kinds: vec![("contentDescription", ResValue::STRING)],
        },
    ])
}

pub const LITERALS: [(AttrFormats, &str, u8); 13] = [
    (AttrFormats::STRING, "true", ResValue::STRING),
    (AttrFormats::STRING, "12", ResValue::STRING),
    (AttrFormats::STRING, "1.5", ResValue::STRING),
    (AttrFormats::STRING, "#abc", ResValue::STRING),
    (AttrFormats::STRING, "16dp", ResValue::STRING),
    (AttrFormats::STRING, "wrap_content", ResValue::STRING),
    (AttrFormats::INTEGER, "12", ResValue::INT_DEC),
    (AttrFormats::INTEGER, "0xff", ResValue::INT_HEX),
    (AttrFormats::FLOAT, "2", ResValue::FLOAT),
    (AttrFormats::BOOLEAN, "false", ResValue::INT_BOOLEAN),
    (AttrFormats::COLOR, "#abc", ResValue::INT_COLOR_RGB8),
    (AttrFormats::DIMENSION, "16dp", ResValue::DIMENSION),
    (AttrFormats::FRACTION, "50%", ResValue::FRACTION),
];
