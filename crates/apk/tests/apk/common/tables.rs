// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::resources::{
    AttrFormats, EntryValue, MapEntry, ResEntry, ResPackage, ResType, ResourceTable, TypeSpec,
};
use reseam_apk::{ResValue, StringPool};
use reseam_dex::DexBytes;

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

pub fn test_strings() -> StringPool {
    strings(&["Hello", "World", "app_name"])
}

/// `string/hello` and `string/world`, at global string 0 and 1.
pub fn test_package() -> ResPackage {
    package(
        "com.example.test",
        &["string"],
        &["hello", "world"],
        [res_type(
            1,
            config(None, 0),
            [
                Some(simple(0, ResValue::STRING, 0)),
                Some(simple(1, ResValue::STRING, 1)),
            ],
        )],
    )
}

pub fn make_test_arsc() -> ResourceTable {
    ResourceTable::new(test_strings(), vec![test_package()])
}

/// A package with id 0x7f holding `types` in order, each type's spec before its first chunk.
pub fn package(
    name: &str,
    type_names: &[&str],
    keys: &[&str],
    types: impl IntoIterator<Item = ResType>,
) -> ResPackage {
    let mut package = ResPackage::new(0x7f, name, strings(type_names), strings(keys));
    let mut specified = Vec::new();
    for res_type in types {
        if !specified.contains(&res_type.id()) {
            specified.push(res_type.id());
            package.add_type_spec(TypeSpec::new(res_type.id(), vec![0; res_type.len()]));
        }
        package.add_type(res_type);
    }
    package
}

/// A configuration chunk of type `id` holding `entries` from index 0, `None` for an absent one.
pub fn res_type(
    id: u8,
    config: Vec<u8>,
    entries: impl IntoIterator<Item = Option<ResEntry>>,
) -> ResType {
    let mut res_type = ResType::new(id, config);
    for (index, entry) in entries.into_iter().enumerate() {
        res_type.set(index, entry).expect("fixture entry index");
    }
    res_type
}

/// A 48-byte `ResTable_config` for `language` and `density`; neither is the default.
pub fn config(language: Option<&[u8; 2]>, density: u16) -> Vec<u8> {
    let mut config = vec![0u8; 48];
    config[0..4].copy_from_slice(&48u32.to_le_bytes());
    if let Some(language) = language {
        config[8..10].copy_from_slice(language);
    }
    config[14..16].copy_from_slice(&density.to_le_bytes());
    config
}

pub fn make_attr_arsc() -> ResourceTable {
    let attr = |key: u32, format: AttrFormats, symbols: [(u32, u32); 2]| {
        Some(ResEntry {
            flags: 0x0001,
            key,
            value: EntryValue::Complex {
                parent: 0,
                entries: std::iter::once(format_item(format))
                    .chain(symbols.map(|(name, value)| MapEntry {
                        name,
                        value: ResValue::int(value as i32),
                    }))
                    .collect(),
            },
        })
    };
    let attrs = [
        attr(0, AttrFormats::ENUM, [(0x7F02_0000, 1), (0x7F02_0001, 2)]),
        attr(1, AttrFormats::FLAGS, [(0x7F02_0002, 1), (0x7F02_0003, 2)]),
    ];
    let ids = (2..6).map(|key| Some(simple(key, ResValue::INT_BOOLEAN, 0)));
    ResourceTable::new(
        strings(&[]),
        vec![package(
            "com.example.attrs",
            &["attr", "id"],
            &["mode", "sides", "compact", "wide", "top", "bottom"],
            [
                res_type(1, config(None, 0), attrs),
                res_type(2, config(None, 0), ids),
            ],
        )],
    )
}

/// The bag item of an `attr` entry declaring its formats.
fn format_item(formats: AttrFormats) -> MapEntry {
    MapEntry {
        name: 0x0100_0000,
        value: ResValue::int(formats.bits() as i32),
    }
}

pub fn table_with_configs(
    type_name: &str,
    entry_name: &str,
    configs: &[(Vec<u8>, EntryValue)],
    global: &[&str],
) -> ResourceTable {
    let types = configs.iter().map(|(config, value)| {
        res_type(
            1,
            config.clone(),
            [Some(ResEntry {
                flags: 0,
                key: 0,
                value: value.clone(),
            })],
        )
    });
    ResourceTable::new(
        strings(global),
        vec![package("com.example", &[type_name], &[entry_name], types)],
    )
}

pub fn attribute_table(definitions: &[(&str, AttrFormats)]) -> ResourceTable {
    let names: Vec<_> = definitions.iter().map(|&(name, _)| name).collect();
    let attrs = definitions.iter().zip(0..).map(|(&(_, formats), key)| {
        Some(ResEntry {
            flags: 0,
            key,
            value: EntryValue::Complex {
                parent: 0,
                entries: vec![format_item(formats)],
            },
        })
    });
    ResourceTable::new(
        strings(&[]),
        vec![package(
            "com.example",
            &["attr"],
            &names,
            [res_type(1, config(None, 0), attrs)],
        )],
    )
}

pub fn round_trip(table: &ResourceTable) -> ResourceTable {
    ResourceTable::parse(DexBytes::from_vec(
        table.serialize().expect("fixture table"),
    ))
    .expect("fixture table")
}
