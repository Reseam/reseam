// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;
use crate::types::access_flags::AccessFlags;
use crate::DexError;

#[test]
fn class_mut_rejects_out_of_bounds_index() {
    let mut dex = DexFile::new(empty_test_header());

    assert!(matches!(
        dex.class_mut(0),
        Err(DexError::Invalid {
            section: "class",
            ..
        })
    ));
}

#[test]
fn intern_descriptors_return_errors() {
    let mut dex = DexFile::new(empty_test_header());

    assert!(matches!(
        dex.intern_proto("(V)V"),
        Err(DexError::Invalid {
            section: "method descriptor",
            ..
        })
    ));
    assert!(matches!(
        dex.intern_method("not-a-type", "name", "()V"),
        Err(DexError::Invalid {
            section: "class descriptor",
            ..
        })
    ));
    assert!(matches!(
        dex.intern_field("Lcom/example/Test;", "value", "bad"),
        Err(DexError::Invalid {
            section: "field descriptor",
            ..
        })
    ));
}

fn dex_with_string_and_class() -> DexFile {
    let mut dex = DexFile::new(empty_test_header());
    dex.intern_string("Lexisting;");
    dex.create_class(
        "Lexisting;",
        crate::types::access_flags::AccessFlags::PUBLIC,
        None,
    )
    .unwrap();
    dex.mark_clean();
    dex
}

#[test]
fn interning_existing_entries_keeps_the_dex_clean() {
    let mut dex = dex_with_string_and_class();
    assert_eq!(dex.intern_string("Lexisting;"), StringIdx(0));
    assert_eq!(dex.intern_type("Lexisting;"), TypeIdx(0));
    assert!(!dex.is_dirty());

    dex.intern_string("new");
    assert!(dex.is_dirty());
}

#[test]
fn class_mut_marks_dirty_and_headers_follow() {
    let mut dex = dex_with_string_and_class();
    assert!(dex.resident_class(0).is_some());
    assert_eq!(dex.class_index_of(TypeIdx(0)), Some(0));

    let super_idx = dex.intern_type("Ljava/lang/Object;");
    dex.mark_clean();
    dex.class_mut(0).unwrap().superclass = Some(super_idx);
    assert!(dex.is_dirty());
    assert_eq!(dex.class_header(0).superclass, Some(super_idx));
    assert_eq!(dex.superclass_chain(0), Vec::<usize>::new());
}

#[test]
fn removing_a_class_drops_it_from_the_type_index() {
    let mut dex = dex_with_string_and_class();
    dex.create_class(
        "Lsecond;",
        crate::types::access_flags::AccessFlags::PUBLIC,
        None,
    )
    .unwrap();
    let second = dex.find_type_idx("Lsecond;").unwrap();
    assert_eq!(dex.class_index_of(second), Some(1));

    assert!(dex.remove_class(TypeIdx(0)).unwrap().is_some());
    assert_eq!(dex.classes.len(), 1);
    assert_eq!(dex.class_index_of(second), Some(0));
    assert_eq!(dex.class_index_of(TypeIdx(0)), None);
}

#[test]
fn write_orders_superclasses_and_interfaces_across_storage_modes() {
    let mut dex = DexFile::new(empty_test_header());
    let child = dex
        .create_class("LAChild;", AccessFlags::PUBLIC, Some("LZBase;"))
        .unwrap();
    let interface = dex.intern_type("LYInterface;");
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
                dex.type_descriptor(dex.class_header(i).class_type)
                    .into_owned()
            })
            .collect::<Vec<_>>()
    };
    let bytes = crate::write(&dex).unwrap();
    let parsed = crate::parse(&bytes, ParseOptions::default()).unwrap();
    assert_eq!(
        descriptors(&parsed),
        ["LZBase;", "LYInterface;", "LAChild;"]
    );
    // Writing does not reorder the source table.
    assert_eq!(descriptors(&dex), ["LAChild;", "LZBase;", "LYInterface;"]);

    for resident_count in [0, 1, 3] {
        for remap in [false, true] {
            let mut dex = crate::parse(
                &bytes,
                ParseOptions {
                    lazy: true,
                    ..ParseOptions::default()
                },
            )
            .unwrap();
            for i in 0..resident_count {
                dex.class_mut(i).unwrap();
            }
            if remap {
                dex.intern_type("L000New;");
            }
            // An external dependency becomes local after parsing; even raw
            // classes must now move after this newly appended definition.
            dex.create_class("Ljava/lang/Object;", AccessFlags::PUBLIC, None)
                .unwrap();
            let written = crate::write(&dex).unwrap();
            assert_eq!(written, crate::write(&dex).unwrap());
            let parsed = crate::parse(&written, ParseOptions::default()).unwrap();
            assert_eq!(
                descriptors(&parsed),
                ["Ljava/lang/Object;", "LZBase;", "LYInterface;", "LAChild;"]
            );
            assert_eq!(crate::write(&parsed).unwrap(), written);
            assert_eq!(dex.classes.iter_resident().count(), resident_count + 1);
        }
    }
}

#[test]
fn write_rejects_cyclic_and_duplicate_class_definitions() {
    for superclass in ["LA;", "LB;"] {
        let mut dex = DexFile::new(empty_test_header());
        dex.create_class("LA;", AccessFlags::PUBLIC, Some(superclass))
            .unwrap();
        let b = dex.create_class("LB;", AccessFlags::PUBLIC, None).unwrap();
        let a = dex.intern_type("LA;");
        dex.class_mut(b).unwrap().interfaces.push(a);
        assert!(
            matches!(crate::write(&dex), Err(DexError::Invalid { section: "class_defs", reason }) if reason.contains("cycle"))
        );
    }

    let mut dex = DexFile::new(empty_test_header());
    for _ in 0..2 {
        dex.create_class("LA;", AccessFlags::PUBLIC, None).unwrap();
    }
    assert!(
        matches!(crate::write(&dex), Err(DexError::Invalid { section: "class_defs", reason }) if reason.contains("duplicate"))
    );
}

#[test]
fn write_keeps_hidden_api_flags_with_reordered_classes() {
    use crate::types::class::EncodedField;
    use crate::types::hidden_api::{ClassHiddenApiFlags, HiddenApiData, HiddenApiFlag};

    let mut dex = DexFile::new(empty_test_header());
    let mut flags = Vec::new();
    for (name, superclass, flag) in [
        ("LZChild;", Some("LABase;"), HiddenApiFlag::Blacklist),
        ("LABase;", None, HiddenApiFlag::Sdk),
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
        flags.push(Some(ClassHiddenApiFlags {
            static_field_flags: vec![flag],
            instance_field_flags: Vec::new(),
            direct_method_flags: Vec::new(),
            virtual_method_flags: Vec::new(),
        }));
    }
    dex.hidden_api = Some(HiddenApiData { class_flags: flags });
    let written = crate::write(&dex).unwrap();
    let parsed = crate::parse(&written, ParseOptions::default()).unwrap();
    let flags = &parsed.hidden_api.as_ref().unwrap().class_flags;
    for (name, expected) in [
        ("LABase;", HiddenApiFlag::Sdk),
        ("LZChild;", HiddenApiFlag::Blacklist),
    ]
    .into_iter()
    {
        let i = parsed.find_class_index(name).unwrap();
        assert_eq!(flags[i].as_ref().unwrap().static_field_flags, [expected]);
    }
}
