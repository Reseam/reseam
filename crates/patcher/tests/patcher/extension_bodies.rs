// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::open_dex_apk;
use crate::common::bundles::{apply, loaded_test_bundle, run_one_patch};
use crate::common::code::{dex_bytes, read_method};
use crate::common::dex;
use reseam_dex::{DexFile, DexHeader, DexVersion};
use reseam_patcher::Patch;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use std::path::{Path, PathBuf};

const BODIES: &str = "Lapp/reseam/test/Bodies;";

/// An extension as Kotlin compiles `external fun`s: `filled`, `missing`, and
/// `unused` are native, and `entry` calls `missing`.
fn bodies_extension(dir: &Path) -> PathBuf {
    use reseam_dex::{AccessFlags, CodeItem, EncodedMethod, Instruction, RegList};

    let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
    let class = dex
        .create_class(BODIES, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let [entry, filled, missing, unused] = ["entry", "filled", "missing", "unused"]
        .map(|name| dex.intern_method(BODIES, name, "()V").unwrap());
    let call_missing = CodeItem::new(
        0,
        0,
        0,
        vec![
            Instruction::InvokeStatic {
                method: missing,
                args: RegList::default(),
            },
            Instruction::ReturnVoid,
        ],
    )
    .unwrap();
    let class = dex.class_mut(class).unwrap();
    class.add_method(EncodedMethod {
        method: entry,
        access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
        code: Some(std::sync::Arc::new(call_missing)),
    });
    for method in [filled, missing, unused] {
        class.add_method(EncodedMethod {
            method,
            access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC | AccessFlags::NATIVE,
            code: None,
        });
    }
    let path = dir.join("bodies.dex");
    std::fs::write(&path, dex_bytes(&dex)).unwrap();
    path
}

#[test]
fn native_extension_methods_the_app_can_reach_need_a_patch_to_implement_them() {
    use reseam_dex::AccessFlags;
    use reseam_patcher::engine::PatchSelection;
    use reseam_patcher::error::PatcherError;

    let dir = tempfile::tempdir().unwrap();
    let extension = bodies_extension(dir.path());
    let bundle = loaded_test_bundle();
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let context = |apk| {
        PatchContext::new(
            apk,
            ExtensionSet::load(std::slice::from_ref(&extension)).unwrap(),
        )
    };

    let (_unfilled_dir, mut apk) = open_dex_apk(&[dex!("entry_values")]);
    let selection = PatchSelection {
        enable: ["unfilled-bodies".to_owned()].into(),
        ..Default::default()
    };
    match apply(&mut context(&mut apk), &patches, &selection) {
        Err(PatcherError::UnimplementedExtensionMethods(methods)) => assert_eq!(
            methods,
            [
                format!("{BODIES}->filled()V"),
                format!("{BODIES}->missing()V")
            ],
            "the hook calls filled, and missing through entry; nothing calls unused"
        ),
        other => panic!(
            "expected unimplemented methods, got {:?}",
            other.map(|_| ())
        ),
    }

    let (_filled_dir, mut apk) = open_dex_apk(&[dex!("entry_values")]);
    run_one_patch(
        &mut context(&mut apk),
        &patches,
        "filled-bodies",
        "filledBodies",
    );
    let linked = apk
        .dex()
        .iter()
        .find(|dex| dex.find_class_index(BODIES).is_some())
        .unwrap();
    for (name, native) in [("filled", false), ("missing", false), ("unused", true)] {
        let method = read_method(linked, BODIES, name).unwrap();
        assert_eq!(
            method.access_flags.contains(AccessFlags::NATIVE),
            native,
            "{name}"
        );
        assert_eq!(method.code.is_some(), !native, "{name}");
    }
}
