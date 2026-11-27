// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::open_dex_apk;
use crate::common::bundles::{loaded_test_bundle, run_one_patch};
use crate::common::code::{dex_bytes, invoked_by};
use reseam_dex::{
    AccessFlags, CodeItem, DexFile, DexHeader, DexVersion, EncodedMethod, Instruction, ParseOptions,
};
use reseam_patcher::Patch;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use std::path::{Path, PathBuf};

const APP: &str = "Lcom/example/App;";
const RENAMED_PACKAGE: &str = "La/b;";
const BUNDLE_PACKAGE: &str = "Lapp/reseam/test/BundlePackage;";
const LIST_PROTO: &str = "(Ljava/util/List;)Ljava/util/List;";

/// An app whose React Native package interface R8 renamed to `a.b`, and whose
/// `getPackages` returns the package list it is given.
fn app_dex() -> Vec<u8> {
    let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
    let interface = dex
        .create_class(
            RENAMED_PACKAGE,
            AccessFlags::PUBLIC | AccessFlags::INTERFACE | AccessFlags::ABSTRACT,
            Some("Ljava/lang/Object;"),
        )
        .unwrap();
    let create_view_managers = dex
        .intern_method(
            RENAMED_PACKAGE,
            "createViewManagers",
            "(Lcom/facebook/react/bridge/ReactApplicationContext;)Ljava/util/List;",
        )
        .unwrap();
    dex.class_mut(interface).unwrap().add_method(EncodedMethod {
        method: create_view_managers,
        access_flags: AccessFlags::PUBLIC | AccessFlags::ABSTRACT,
        code: None,
    });
    let app = dex
        .create_class(APP, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let get_packages = dex.intern_method(APP, "getPackages", LIST_PROTO).unwrap();
    let body = CodeItem::new(1, 1, 0, vec![Instruction::ReturnObject { src: 0 }]).unwrap();
    dex.class_mut(app).unwrap().add_method(EncodedMethod {
        method: get_packages,
        access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
        code: Some(std::sync::Arc::new(body)),
    });
    dex_bytes(&dex)
}

/// The bundle's package: a class that names no React Native type, with a
/// static `addTo`.
fn package_extension(dir: &Path) -> PathBuf {
    let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
    let class = dex
        .create_class(
            BUNDLE_PACKAGE,
            AccessFlags::PUBLIC,
            Some("Ljava/lang/Object;"),
        )
        .unwrap();
    let add_to = dex
        .intern_method(BUNDLE_PACKAGE, "addTo", "(Ljava/util/List;)V")
        .unwrap();
    let body = CodeItem::new(1, 1, 0, vec![Instruction::ReturnVoid]).unwrap();
    dex.class_mut(class).unwrap().add_method(EncodedMethod {
        method: add_to,
        access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
        code: Some(std::sync::Arc::new(body)),
    });
    let path = dir.join("package.dex");
    std::fs::write(&path, dex_bytes(&dex)).unwrap();
    path
}

#[test]
fn a_bundle_package_implements_the_apps_renamed_package_interface() {
    let dir = tempfile::tempdir().unwrap();
    let extension = package_extension(dir.path());
    let bundle = loaded_test_bundle();
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let (_apk_dir, mut apk) = open_dex_apk(&[&app_dex()]);
    run_one_patch(
        &mut PatchContext::new(
            &mut apk,
            ExtensionSet::load(std::slice::from_ref(&extension)).unwrap(),
        ),
        &patches,
        "bundle-react-package",
        "bundleReactPackage",
    );
    let dex = apk.dex().get(0).unwrap();
    assert!(
        invoked_by(dex, APP, "getPackages").contains(&format!("{BUNDLE_PACKAGE}->addTo")),
        "getPackages calls the package's addTo"
    );
    let linked = apk
        .dex()
        .iter()
        .find(|dex| dex.find_class_index(BUNDLE_PACKAGE).is_some())
        .unwrap();
    let class = linked
        .classes()
        .read(
            linked.find_class_index(BUNDLE_PACKAGE).unwrap(),
            ParseOptions::default(),
        )
        .unwrap();
    let interfaces: Vec<_> = class
        .interfaces
        .iter()
        .map(|&interface| linked.type_descriptor(interface).into_owned())
        .collect();
    assert_eq!(interfaces, [RENAMED_PACKAGE]);
}
