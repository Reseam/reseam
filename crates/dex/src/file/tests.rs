// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod metadata;
mod rollback;

use super::*;
use crate::model::access_flags::AccessFlags;

fn write(dex: &DexFile) -> crate::Result<Vec<u8>> {
    read(&crate::write_spooled(dex, None)?.0)
}

fn read(spooled: &crate::Spooled) -> crate::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut spooled.reader(), &mut bytes)?;
    Ok(bytes)
}

#[test]
fn write_orders_superclasses_and_interfaces_across_storage_modes() {
    let mut dex = DexFile::new(DexHeader::new(crate::DexVersion::V035));
    let child = dex
        .create_class("LAChild;", AccessFlags::PUBLIC, Some("LZBase;"))
        .unwrap();
    let interface = dex
        .intern_type("LYInterface;")
        .expect("valid fixture descriptor");
    dex.class_mut(child).unwrap().interfaces.push(interface);
    dex.create_class("LZBase;", AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    dex.create_class(
        "LYInterface;",
        AccessFlags::PUBLIC | AccessFlags::INTERFACE | AccessFlags::ABSTRACT,
        Some("Ljava/lang/Object;"),
    )
    .unwrap();

    let descriptors = |dex: &DexFile| {
        (0..dex.classes.len())
            .map(|i| {
                dex.type_descriptor(dex.classes().header(i).class_type)
                    .into_owned()
            })
            .collect::<Vec<_>>()
    };
    let bytes = write(&dex).unwrap();
    let parsed = crate::parse(&bytes, ParseOptions::default()).unwrap();
    assert_eq!(
        descriptors(&parsed),
        ["LZBase;", "LYInterface;", "LAChild;"]
    );

    for resident_count in [0, 1, 3] {
        for remap in [false, true] {
            let mut dex = crate::parse(
                &bytes,
                ParseOptions {
                    classes: crate::options::Loading::Deferred,
                    ..ParseOptions::default()
                },
            )
            .unwrap();
            for i in 0..resident_count {
                dex.class_mut(i).unwrap();
            }
            if remap {
                dex.intern_type("L000New;")
                    .expect("valid fixture descriptor");
            }
            // An external dependency becomes local after parsing; even raw
            // classes must now move after this newly appended definition.
            dex.create_class("Ljava/lang/Object;", AccessFlags::PUBLIC, None)
                .unwrap();
            let written = write(&dex).unwrap();
            assert_eq!(written, write(&dex).unwrap());
            let parsed = crate::parse(&written, ParseOptions::default()).unwrap();
            assert_eq!(
                descriptors(&parsed),
                ["Ljava/lang/Object;", "LZBase;", "LYInterface;", "LAChild;"]
            );
            assert_eq!(write(&parsed).unwrap(), written);
        }
    }
}

#[test]
fn write_rejects_cyclic_and_duplicate_class_definitions() {
    for superclass in ["LA;", "LB;"] {
        let mut dex = DexFile::new(DexHeader::new(crate::DexVersion::V035));
        dex.create_class("LA;", AccessFlags::PUBLIC, Some(superclass))
            .unwrap();
        let b = dex.create_class("LB;", AccessFlags::PUBLIC, None).unwrap();
        let a = dex.intern_type("LA;").expect("valid fixture descriptor");
        dex.class_mut(b).unwrap().interfaces.push(a);
        assert!(write(&dex).is_err());
    }

    let mut dex = DexFile::new(DexHeader::new(crate::DexVersion::V035));
    for _ in 0..2 {
        dex.create_class("LA;", AccessFlags::PUBLIC, None).unwrap();
    }
    assert!(write(&dex).is_err());
}

#[test]
fn write_keeps_hidden_api_flags_with_reordered_classes() {
    use crate::model::class::EncodedField;
    use crate::{ClassHiddenApiFlags, HiddenApiData, HiddenApiFlags};

    let mut dex = DexFile::new(DexHeader::new(crate::DexVersion::V035));
    let mut flags = std::collections::BTreeMap::new();
    for (name, (superclass, flag)) in [
        (
            "LZChild;",
            (Some("LABase;"), HiddenApiFlags::from_bits(0x37)),
        ),
        ("LABase;", (None, HiddenApiFlags::SDK)),
    ] {
        let class = dex
            .create_class(name, AccessFlags::PUBLIC, superclass)
            .unwrap();
        let field = dex.intern_field(name, "value", "I").unwrap();
        dex.class_mut(class)
            .unwrap()
            .add_static_field(EncodedField {
                field,
                access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
            });
        flags.insert(
            dex.classes().header(class).class_type,
            ClassHiddenApiFlags {
                field_flags: [(field, flag)].into_iter().collect(),
                method_flags: std::collections::BTreeMap::default(),
            },
        );
    }
    dex.hidden_api = Some(HiddenApiData::from_flags(flags));
    let written = write(&dex).unwrap();
    let parsed = crate::parse(&written, ParseOptions::default()).unwrap();
    let flags = parsed.hidden_api.as_ref().unwrap();
    for (name, expected) in [
        ("LABase;", HiddenApiFlags::SDK),
        ("LZChild;", HiddenApiFlags::from_bits(0x37)),
    ] {
        let i = parsed.find_class_index(name).unwrap();
        let ty = parsed.classes().header(i).class_type;
        assert_eq!(
            flags
                .get(ty)
                .expect("class flags decode")
                .expect("class flags exist")
                .field_flags
                .values()
                .copied()
                .collect::<Vec<_>>(),
            [expected]
        );
    }
    let deferred = crate::parse(
        &written,
        ParseOptions {
            classes: crate::Loading::Deferred,
            ..ParseOptions::default()
        },
    )
    .expect("deferred classes parse");
    let output = write(&deferred).expect("source flags serialize");
    let reparsed = crate::parse(&output, ParseOptions::default()).expect("flags parse");
    for (name, expected) in [
        ("LABase;", HiddenApiFlags::SDK),
        ("LZChild;", HiddenApiFlags::from_bits(0x37)),
    ] {
        let ty = reparsed.find_type_idx(name).expect("class type");
        assert_eq!(
            reparsed
                .hidden_api()
                .expect("hidden API")
                .get(ty)
                .expect("flags decode")
                .expect("class flags")
                .field_flags
                .values()
                .copied()
                .collect::<Vec<_>>(),
            [expected]
        );
    }
}

#[test]
fn static_values_and_flags_follow_members_through_sorting_and_removal() {
    use crate::{ClassHiddenApiFlags, EncodedField, EncodedValue, HiddenApiData, HiddenApiFlags};
    let mut dex = DexFile::new(DexHeader::new(crate::DexVersion::V035));
    let class = dex
        .create_class("LFields;", AccessFlags::PUBLIC, None)
        .unwrap();
    let z = dex.intern_field("LFields;", "z", "I").unwrap();
    let a = dex.intern_field("LFields;", "a", "I").unwrap();
    for field in [z, a] {
        dex.class_mut(class)
            .unwrap()
            .add_static_field(EncodedField {
                field,
                access_flags: AccessFlags::STATIC,
            });
    }
    dex.class_mut(class)
        .unwrap()
        .static_values
        .extend([(z, EncodedValue::Int(11)), (a, EncodedValue::Int(22))]);
    dex.hidden_api = Some(HiddenApiData::from_flags(
        [(
            dex.classes().header(class).class_type,
            ClassHiddenApiFlags {
                field_flags: [
                    (z, HiddenApiFlags::BLOCKED),
                    (a, HiddenApiFlags::from_bits(0x28)),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
    ));
    for remove in [false, true] {
        if remove {
            dex.class_mut(class)
                .unwrap()
                .class_data
                .as_mut()
                .unwrap()
                .static_fields
                .retain(|field| field.field != z);
        }
        let bytes = write(&dex).unwrap();
        let parsed = crate::parse(&bytes, ParseOptions::default()).unwrap();
        let data = parsed
            .classes()
            .resident(0)
            .unwrap()
            .class_data
            .as_ref()
            .unwrap();
        for field in &data.static_fields {
            let name = parsed.strings().get(parsed.field_id(field.field).name);
            let (value, flag) = if name == "a" {
                (22, HiddenApiFlags::from_bits(0x28))
            } else {
                (11, HiddenApiFlags::BLOCKED)
            };
            assert_eq!(
                parsed
                    .classes()
                    .resident(0)
                    .unwrap()
                    .static_values
                    .get(&field.field),
                Some(&EncodedValue::Int(value))
            );
            assert_eq!(
                parsed
                    .hidden_api
                    .as_ref()
                    .unwrap()
                    .get(parsed.classes().header(0).class_type)
                    .expect("flags decode")
                    .expect("flags exist")
                    .field_flags[&field.field],
                flag
            );
        }
        assert_eq!(data.static_fields.len(), if remove { 1 } else { 2 });
    }
}

#[test]
fn container_members_are_finalized_after_layout_and_require_complete_input() {
    let mut first = DexFile::new(DexHeader::new(crate::DexVersion::V035));
    first
        .create_class("LFirst;", AccessFlags::PUBLIC, None)
        .unwrap();
    let mut second = DexFile::new(DexHeader::new(crate::DexVersion::V035));
    second
        .create_class("LSecond;", AccessFlags::PUBLIC, None)
        .unwrap();
    let bytes = read(
        &crate::write_container_spooled([(&first, None), (&second, None)])
            .unwrap()
            .0,
    )
    .unwrap();
    let members = crate::parse_container(&bytes, ParseOptions::default()).unwrap();
    assert_eq!(members.len(), 2);
    assert!(
        members
            .iter()
            .all(|dex| dex.header.version == crate::DexVersion::V041)
    );
    assert_eq!(
        members[0].type_descriptor(members[0].classes().header(0).class_type),
        "LFirst;"
    );
    assert_eq!(
        members[1].type_descriptor(members[1].classes().header(0).class_type),
        "LSecond;"
    );
    let mut damaged = bytes.clone();
    damaged[members[1].header.header_offset as usize] = 0;
    let options = ParseOptions {
        checksum: crate::options::Verification::Skip,
        signature: crate::options::Verification::Skip,
        ..ParseOptions::default()
    };
    assert!(crate::parse_container(&damaged, options).is_err());
    assert!(crate::parse_container(&bytes[..bytes.len() - 1], options).is_err());
}
