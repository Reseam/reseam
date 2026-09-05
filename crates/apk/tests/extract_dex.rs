// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use reseam_apk::reseam_dex::ParseOptions;

const YOUTUBE_APK: &str = "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";

#[test]
fn extract_dex_returns_entries_in_order() {
    if !Path::new(YOUTUBE_APK).exists() {
        return;
    }
    let (container, names) =
        reseam_apk::extract_dex(Path::new(YOUTUBE_APK), ParseOptions::default()).expect("extract");
    assert!(!container.is_empty());
    assert_eq!(names.len(), container.len());
    assert_eq!(names[0].as_str(), "classes.dex");
}
