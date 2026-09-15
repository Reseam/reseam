// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use reseam_apk::reseam_dex::{self as dex, ParseOptions};
use reseam_apk::ApkFile;
use reseam_patcher::context::{MethodKey, PatchContext};

const APK: &str = "../../test-apks/com.Splitwise.SplitwiseMobile_26.5.3.apk";

const LOG_D: MethodKey<'static> = MethodKey {
    class: "Landroid/util/Log;",
    name: "d",
    proto: "(Ljava/lang/String;Ljava/lang/String;)I",
};

const REDIRECTED: MethodKey<'static> = MethodKey {
    class: "Lapp/test/Logger;",
    name: "d",
    proto: "(Ljava/lang/String;Ljava/lang/String;)I",
};

#[test]
fn redirects_every_call_site_once() {
    if !Path::new(APK).exists() {
        return;
    }
    let mut apk = ApkFile::open(APK, &ApkFile::patch_options()).unwrap();
    let mut ctx = PatchContext::new(&mut apk);

    let changed = ctx.redirect_method_calls(LOG_D, REDIRECTED);
    assert!(changed > 0, "the app never calls Log.d");

    let targets = [(REDIRECTED.class.to_owned(), REDIRECTED.name.to_owned())];
    assert_eq!(ctx.find_method_call_sites(&targets).len(), changed);
    assert_eq!(ctx.redirect_method_calls(LOG_D, REDIRECTED), 0);
}

#[test]
fn leaves_calls_in_added_dex_files() {
    if !Path::new(APK).exists() {
        return;
    }
    let expected = {
        let mut apk = ApkFile::open(APK, &ApkFile::patch_options()).unwrap();
        PatchContext::new(&mut apk).redirect_method_calls(LOG_D, REDIRECTED)
    };
    let mut apk = ApkFile::open(APK, &ApkFile::patch_options()).unwrap();
    let copy = dex::write(apk.dex().dex(0).unwrap()).unwrap();
    apk.add_dex(dex::parse(&copy, ParseOptions::default()).unwrap());
    let added = apk.dex().iter().count() - 1;
    let mut ctx = PatchContext::new(&mut apk);

    let log_d = [(LOG_D.class.to_owned(), LOG_D.name.to_owned())];
    let calls_in_added = |ctx: &PatchContext| {
        ctx.find_method_call_sites(&log_d)
            .iter()
            .filter(|site| site.loc.method.dex_idx == added)
            .count()
    };
    let before = calls_in_added(&ctx);
    assert!(before > 0, "the copied DEX never calls Log.d");

    assert_eq!(ctx.redirect_method_calls(LOG_D, REDIRECTED), expected);
    assert_eq!(calls_in_added(&ctx), before);
}
