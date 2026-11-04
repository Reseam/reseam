// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::open_dex_apk;
use crate::common::bundles::{apply, apply_fixture, loaded_test_bundle};
use crate::common::code::{hook_calls, patched_code};
use crate::common::dex;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use reseam_patcher::engine::{PatchSelection, PatchStatus};

const OWNER: &str = "Lcom/example/AnchorHost;";

#[test]
fn entry_hooks_keep_registration_order_across_patches_and_finalization() {
    for include_failure in [false, true] {
        let (_directory, mut apk) = open_dex_apk(&[dex!("point_order")]);
        let bundle = loaded_test_bundle();
        let mut enable = vec!["entry-hook-dependent".to_owned()];
        if include_failure {
            enable.push("entry-hook-failure".to_owned());
        }
        let results = apply(
            &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
            &bundle.patches().iter().collect::<Vec<_>>(),
            &PatchSelection {
                enable,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result.status, PatchStatus::Failed { .. }))
                .count(),
            usize::from(include_failure),
            "{results:?}"
        );
        assert_eq!(
            hook_calls(&patched_code(&apk, OWNER, "twice"), &[]),
            (vec![vec![10], vec![20], vec![77], vec![30], vec![40]], None),
            "include_failure={include_failure}"
        );
    }
}

#[test]
fn replacing_the_body_resets_the_entry_hook_position() {
    let (_directory, mut apk) = open_dex_apk(&[dex!("point_order")]);
    apply_fixture(&mut apk, "entry-hook-replacement", "entryHookReplacement");
    assert_eq!(
        hook_calls(&patched_code(&apk, OWNER, "twice"), &[]),
        (vec![vec![20], vec![25], vec![30]], None)
    );
}
