// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::table_bytes::*;
use crate::common::table_cases;
use crate::common::tables::*;
use reseam_apk::resources::{AttrFormats, EntryValue, MapEntry, ResourceTable, StyleItem};
use reseam_apk::{ResValue, ResourceScope};
use reseam_dex::DexBytes;

#[test]
fn lazy_resource_reads_and_bag_edits_report_corruption() -> TestResult {
    for corrupt_pool in [false, true] {
        let mut bytes = table_with_configs(
            "array",
            "values",
            &[(
                config(None, 0),
                EntryValue::Complex {
                    parent: 0,
                    entries: vec![MapEntry {
                        name: 0x0100_0001,
                        value: ResValue::string(0),
                    }],
                },
            )],
            &["original"],
        )
        .serialize()?;
        let offset = if corrupt_pool {
            chunk(&bytes, 12..bytes.len(), 1).start + 28
        } else {
            let pkg = chunk(&bytes, 12..bytes.len(), 0x200);
            let t = chunk(&bytes, pkg.start + 288..pkg.end, 0x201);
            t.start + u32_at(&bytes, t.start + 16) as usize + 12
        };
        bytes[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut table = ResourceTable::parse(DexBytes::from_vec(bytes))?;
        if corrupt_pool {
            assert!(table.get_string(0).is_err());
            assert!(table.array("values").is_err());
        } else {
            assert!(table.array_values("values").is_err());
            let before = table.serialize()?;
            assert!(
                table
                    .set_array_values("values", &[ResValue::int(5)])
                    .is_err()
            );
            assert_eq!(table.serialize()?, before);
        }
    }
    Ok(())
}

#[test]
fn staged_resource_aliases_resolve_ids_without_rewriting_metadata() -> TestResult {
    for header in [12, 16] {
        let bytes = alias_resource_table(header, 2);
        let table = ResourceTable::parse(DexBytes::from_slice(&bytes))?;
        for (id, expected) in [
            (0x7fff_0000, 0x7f01_0000),
            (0x7ffe_0000, 0x7f01_0001),
            (0x7f01_0000, 0x7f01_0000),
        ] {
            assert_eq!(table.finalized_resource_id(id)?, expected);
        }
        assert_eq!(table.serialize()?, bytes);
    }
    for bytes in [extended_resource_table(), alias_resource_table(12, 3)] {
        assert!(
            ResourceTable::parse(DexBytes::from_vec(bytes))?
                .finalized_resource_id(0x7fff_0000)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn resource_edits_reject_invalid_indices_without_changing_the_table() -> TestResult {
    let mut entries = res_type(1, config(None, 0), [Some(simple(0, ResValue::STRING, 0))]);
    for index in [65_536, usize::MAX] {
        assert!(entries.set(index, None).is_err());
        assert_eq!(entries.len(), 1);
        assert!(entries.entry(index)?.is_none());
        assert_eq!(entries.entry(0)?.expect("entry").key, 0);
    }
    let mut table = table_with_configs(
        "array",
        "items",
        &[(
            config(None, 0),
            EntryValue::Complex {
                parent: 0,
                entries: Vec::new(),
            },
        )],
        &[],
    );
    let original = table.serialize()?;
    assert!(
        table
            .set_array_values("items", &[ResValue::string(u32::MAX)])
            .is_err()
    );
    assert_eq!(table.serialize()?, original);
    Ok(())
}

#[test]
fn resource_lookup_and_string_replacement_survive_serialization() -> TestResult {
    for (index, case) in table_cases::cases()?.into_iter().enumerate() {
        let mut table = ResourceTable::parse(DexBytes::from_slice(&case.input))?;
        assert_eq!(table.serialize()?, case.input, "unchanged case {index}");
        (case.edit)(&mut table)?;
        assert_eq!(
            (case.read)(&round_trip(&table))?,
            case.expected,
            "edited case {index}"
        );
    }
    Ok(())
}

#[test]
fn entries_are_found_by_the_string_they_hold_and_named_by_id() -> TestResult {
    let table = ResourceTable::parse(DexBytes::from_vec(make_test_arsc().serialize()?))?;
    let entries = table.string_entries()?;

    for (string, key) in [(0, "hello"), (1, "world")] {
        let [found] = table
            .find_entries_by_string(string)?
            .try_into()
            .expect("one entry");
        assert_eq!(found.key_name, key);
        assert_eq!(
            entries.get(string),
            [reseam_apk::resources::StringUse {
                res_id: found.res_id,
                qualifiers: String::new(),
            }]
        );
        assert_eq!(
            table
                .resource_name(found.res_id)?
                .map(|name| name.to_string()),
            Some(format!("@string/{key}"))
        );
    }
    assert!(entries.get(2).is_empty());
    assert_eq!(table.resource_name(0x7f01_0005)?, None);
    Ok(())
}

#[test]
fn fixed_ids_added_attributes_and_style_items_read_back_by_name() -> TestResult {
    let mut table = make_test_arsc();
    let hello = table
        .find_resource_id("string", "hello")?
        .expect("fixture id");
    let fixed = hello + 0x10;
    let color = ResValue::new(ResValue::INT_COLOR_ARGB8, 0xff11_2233);
    for (name, id) in [
        ("world", fixed),
        ("later", hello),
        ("later", fixed + 0x0001_0000),
    ] {
        assert!(
            table
                .add_resource_at("string", name, color, "", id)
                .is_err(),
            "string/{name} at {id:#010x} should be refused"
        );
    }
    assert_eq!(
        table.add_resource_at("string", "later", color, "", fixed)?,
        fixed
    );
    let accent = table.add_attr("accent", AttrFormats::parse("reference | color")?)?;
    assert!(AttrFormats::parse("colour").is_err());
    ResourceScope::from(&mut table).set_style_items(
        "Theme",
        Some("@android:style/Theme"),
        &[
            StyleItem {
                attribute: "accent".into(),
                value: "@string/later".into(),
            },
            StyleItem {
                attribute: "android:textColor".into(),
                value: "#ff000000".into(),
            },
            StyleItem {
                attribute: "android:windowBackground".into(),
                value: "@empty".into(),
            },
        ],
    )?;

    let table = round_trip(&table);
    assert_eq!(
        table.attr_formats(accent)?,
        Some(AttrFormats::REFERENCE | AttrFormats::COLOR)
    );
    assert_eq!(
        table.resource_names("string")?,
        ["hello", "world", "later"].map(String::from)
    );
    assert_eq!(
        table.style_items("Theme", "")?,
        Some(vec![
            StyleItem {
                attribute: "android:windowBackground".into(),
                value: "@empty".into(),
            },
            StyleItem {
                attribute: "android:textColor".into(),
                value: "#ff000000".into(),
            },
            StyleItem {
                attribute: "accent".into(),
                value: "@string/later".into(),
            },
        ])
    );
    assert_eq!(table.style_items("Theme", "night")?, None);
    Ok(())
}
