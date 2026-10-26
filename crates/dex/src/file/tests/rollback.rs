// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_journal::Savepoint;

use super::*;
use crate::{RefKey, RefQuery};

#[test]
fn rollback_returns_a_dex_to_each_savepoint() {
    let fixture = || {
        let mut dex = DexFile::new(DexHeader::new(crate::DexVersion::V035));
        for name in ["LA;", "LB;", "LC;"] {
            let class = dex
                .create_class(name, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
                .unwrap();
            let method = dex.intern_method(name, "run", "()V").unwrap();
            dex.class_mut(class)
                .unwrap()
                .add_method(crate::EncodedMethod {
                    method,
                    access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
                    code: Some(std::sync::Arc::new(
                        crate::CodeItem::new(0, 0, 0, vec![crate::Instruction::ReturnVoid])
                            .unwrap(),
                    )),
                });
        }
        write(&dex).unwrap()
    };
    let bytes = fixture();
    let mut dex = crate::parse(
        &bytes,
        ParseOptions {
            classes: crate::options::Loading::Deferred,
            ..ParseOptions::default()
        },
    )
    .unwrap();
    let original = write(&dex).unwrap();

    let outer = Savepoint::FIRST;
    dex.open_savepoint(outer);
    dex.intern_string("added");
    dex.set_superclass(1, "LC;").unwrap();
    dex.create_class("LD;", AccessFlags::PUBLIC, Some("LA;"))
        .unwrap();
    let after_outer = write(&dex).unwrap();

    let inner = outer.next();
    dex.open_savepoint(inner);
    let removed = dex.intern_type("LB;").unwrap();
    dex.remove_class(removed).unwrap();
    dex.intern_field("LA;", "count", "I").unwrap();
    dex.class_mut(0).unwrap().access_flags |= AccessFlags::FINAL;
    dex.open_savepoint(inner.next());
    dex.intern_string("released");
    dex.release_savepoint(inner.next());

    dex.rollback_to(inner);
    assert_eq!(write(&dex).unwrap(), after_outer);
    dex.rollback_to(outer);
    assert_eq!(write(&dex).unwrap(), original);
    assert!(!dex.is_dirty());
    assert_eq!(dex.find_class_index("LB;"), Some(1));
}

#[test]
fn searches_find_a_class_a_rollback_returned_to_its_file_record() {
    let mut built = DexFile::new(DexHeader::new(crate::DexVersion::V035));
    let marker = built.intern_string("marker");
    let class = built
        .create_class("LA;", AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let method = built.intern_method("LA;", "run", "()V").unwrap();
    built
        .class_mut(class)
        .unwrap()
        .add_method(crate::EncodedMethod {
            method,
            access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
            code: Some(std::sync::Arc::new(
                crate::CodeItem::new(
                    1,
                    0,
                    0,
                    vec![
                        crate::Instruction::ConstString {
                            dest: 0,
                            string: marker,
                        },
                        crate::Instruction::ReturnVoid,
                    ],
                )
                .unwrap(),
            )),
        });
    let bytes = write(&built).unwrap();
    let mut dex = crate::parse(
        &bytes,
        ParseOptions {
            classes: crate::options::Loading::Deferred,
            ..ParseOptions::default()
        },
    )
    .unwrap();
    let marker = dex.strings().find("marker").unwrap();
    let query = RefQuery::all_of([RefKey::String(marker)]);
    let found = |dex: &DexFile| {
        dex.scan_methods_collect(&query, |view| Ok(Some(view.class_idx)))
            .unwrap()
    };

    dex.open_savepoint(Savepoint::FIRST);
    dex.class_mut(0).unwrap();
    assert_eq!(found(&dex), [0]);
    dex.rollback_to(Savepoint::FIRST);
    assert_eq!(found(&dex), [0]);
}
