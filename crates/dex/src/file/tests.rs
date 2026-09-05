// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;
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
