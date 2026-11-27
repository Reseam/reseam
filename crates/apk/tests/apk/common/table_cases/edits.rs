// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Case, Readback};
use crate::common::table_bytes::*;
use crate::common::tables::*;
use reseam_apk::resources::{EntryValue, MapEntry, StyleItem};
use reseam_apk::{ResValue, ResourceScope};

pub(super) fn style_case() -> reseam_apk::Result<Case> {
    let mut night = config(None, 0);
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
            &[(config(None, 0), existing.clone()), (night, existing)],
            &[],
        )
        .serialize()?,
        edit: Box::new(|table| {
            ResourceScope::from(table).set_style_items(
                "AppTheme",
                None,
                &[
                    StyleItem {
                        attribute: "android:windowBackground".into(),
                        value: "#ffffffff".into(),
                    },
                    StyleItem {
                        attribute: "android:textColor".into(),
                        value: "#ff102030".into(),
                    },
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

pub(super) fn night_style_case() -> reseam_apk::Result<Case> {
    let text_color = MapEntry {
        name: 0x0101_0098,
        value: ResValue::new(ResValue::INT_COLOR_ARGB8, 0xff00_0000),
    };
    Ok(Case {
        input: table_with_configs(
            "style",
            "AppTheme",
            &[(
                config(None, 0),
                EntryValue::Complex {
                    parent: 0x0103_0005,
                    entries: vec![text_color],
                },
            )],
            &[],
        )
        .serialize()?,
        edit: Box::new(|table| {
            ResourceScope::from(table).set_style_items_in(
                "AppTheme",
                None,
                &[StyleItem {
                    attribute: "android:windowBackground".into(),
                    value: "#ffffffff".into(),
                }],
                "night",
            )?;
            Ok(())
        }),
        read: Box::new(|table| {
            let mut result = Readback::default();
            for config in table.packages()[0].types() {
                let EntryValue::Complex { parent, entries } =
                    config.entry(0)?.expect("fixture style").value
                else {
                    panic!("bag");
                };
                result.numbers.push(u32::from(config.is_default_config()));
                result.numbers.push(parent);
                result.numbers.extend(entries.iter().map(|e| e.name));
            }
            Ok(result)
        }),
        expected: Readback {
            numbers: vec![
                1,
                0x0103_0005,
                0x0101_0098,
                0,
                0x0103_0005,
                0x0101_0054,
                0x0101_0098,
            ],
            ..Default::default()
        },
    })
}

pub(super) fn new_array_case() -> reseam_apk::Result<Case> {
    Ok(Case {
        input: table_with_configs(
            "string",
            "label",
            &[(config(None, 0), EntryValue::Simple(ResValue::string(0)))],
            &["label"],
        )
        .serialize()?,
        edit: Box::new(|table| {
            table.set_string_array("added", &["one", "2"].map(String::from))?;
            Ok(())
        }),
        read: Box::new(|table| {
            Ok(Readback {
                texts: table.array("added")?,
                numbers: vec![table.find_resource_id("array", "added")?.expect("array id")],
                ..Default::default()
            })
        }),
        expected: Readback {
            texts: vec!["one".into(), "2".into()],
            numbers: vec![0x7f02_0000],
            ..Default::default()
        },
    })
}

pub(super) fn array_case() -> reseam_apk::Result<Case> {
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
                config(None, 0),
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

pub(super) fn file_cases() -> reseam_apk::Result<Vec<Case>> {
    let mut cases = Vec::new();
    for initial in [config(None, 0), config(None, 480)] {
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
                    texts: table
                        .files("mipmap", "launcher")?
                        .into_iter()
                        .map(|file| file.path)
                        .collect(),
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
                    .push(table.files("mipmap", "reseam_icon")?.len() as u32);
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
