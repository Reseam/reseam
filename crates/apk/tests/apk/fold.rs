// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::dex::{class_dex, dex_bytes};
use crate::common::session::*;
use reseam_apk::{ApkFile as Apk, ApkWriteOptions as WriteOptions};
use reseam_dex::{AccessFlags, EncodedMethod};
use reseam_journal::Savepoint;

/// The classes each DEX of `apk` defines, in load order.
fn classes_by_dex(apk: &Apk) -> Vec<Vec<String>> {
    apk.dex()
        .iter()
        .map(|dex| {
            dex.classes()
                .headers()
                .map(|header| dex.type_descriptor(header.class_type).into_owned())
                .collect()
        })
        .collect()
}

#[test]
fn folding_moves_added_classes_into_app_dex_while_their_ids_fit() -> TestResult {
    for (unused_protos, expected) in [
        (0u32, vec![vec!["LApp;", "LAdded;"]]),
        (1 << 16, vec![vec!["LApp;"], vec!["LAdded;"]]),
    ] {
        let Session { dir, mut apk } =
            Session::single(&[("classes.dex", &dex_bytes(&class_dex("LApp;")))]);
        let app = apk.dex_mut(0).expect("app DEX");
        for value in 0..unused_protos {
            let parameters: String = (0..16)
                .map(|bit| if value & (1 << bit) == 0 { 'B' } else { 'I' })
                .collect();
            app.intern_proto(&format!("({parameters})V"))?;
        }
        apk.add_dex(class_dex("LAdded;"));
        apk.fold_dex()?;
        let paths = apk.write_to(dir.path().join("out"), WriteOptions::default())?;
        assert_eq!(classes_by_dex(&Apk::open(&paths[0], lazy())?), expected);
    }
    Ok(())
}

#[test]
fn folding_moves_classes_an_edited_dex_has_no_room_for_into_another() -> TestResult {
    let mut crowded = class_dex("LFirst;");
    crowded.create_class("LSecond;", AccessFlags::PUBLIC, None)?;
    let Session { dir, mut apk } = Session::single(&[
        ("classes.dex", &dex_bytes(&crowded)),
        ("classes2.dex", &dex_bytes(&class_dex("LRoomy;"))),
    ]);
    let dex = apk.dex_mut(0).expect("crowded DEX");
    for (class, descriptor) in ["LFirst;", "LSecond;"].into_iter().enumerate() {
        for number in 0..40_000 {
            let method = dex.intern_method(descriptor, &format!("call{number}"), "()V")?;
            dex.class_mut(class)?.add_method(EncodedMethod {
                method,
                access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC | AccessFlags::NATIVE,
                code: None,
            });
        }
    }
    apk.fold_dex()?;
    let paths = apk.write_to(dir.path().join("out"), WriteOptions::default())?;
    assert_eq!(
        classes_by_dex(&Apk::open(&paths[0], lazy())?),
        [vec!["LFirst;"], vec!["LRoomy;", "LSecond;"]]
    );
    Ok(())
}

#[test]
fn a_fold_request_is_undone_with_the_savepoint_it_was_made_under() {
    let Session { mut apk, .. } =
        Session::single(&[("classes.dex", &dex_bytes(&class_dex("LApp;")))]);
    apk.open_savepoint(Savepoint::FIRST);
    apk.request_dex_folding();
    assert!(apk.dex_folding_requested());
    apk.rollback_to(Savepoint::FIRST);
    assert!(!apk.dex_folding_requested());
}
