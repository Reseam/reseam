// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::{entry, open_dex_apk};
use crate::common::bundles::apply_fixture;
use crate::common::code::{hook_calls, patched_code};
use crate::common::dex;

#[test]
fn point_before_runs_on_branches_into_the_instruction() {
    const OWNER: &str = "Lcom/example/SkipHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("branch_entry")]);
    apply_fixture(&mut apk, "before-join", "beforeJoin");

    let code = patched_code(&apk, OWNER, "joinedBefore");
    assert_eq!(
        hook_calls(&code, &[0]),
        (vec![vec![1], vec![9], vec![2], vec![3]], None)
    );
    assert_eq!(
        hook_calls(&code, &[1]),
        (vec![vec![1], vec![4], vec![9], vec![4], vec![3]], None)
    );
}

#[test]
fn skip_when_guards_a_call_on_every_path_into_it() {
    const OWNER: &str = "Lcom/example/SkipHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("guarded_calls")]);
    apply_fixture(&mut apk, "skip-when", "skipWhenCall");

    let straight = patched_code(&apk, OWNER, "straight");
    assert_eq!(hook_calls(&straight, &[]), (vec![vec![1], vec![3]], None));
    let joined = patched_code(&apk, OWNER, "joined");
    assert_eq!(hook_calls(&joined, &[0]), (vec![vec![1], vec![3]], None));
    assert_eq!(
        hook_calls(&joined, &[1]),
        (vec![vec![1], vec![4], vec![3]], None)
    );
    let kept = patched_code(&apk, OWNER, "joinedKept");
    assert_eq!(
        hook_calls(&kept, &[0]),
        (vec![vec![1], vec![2], vec![3]], None)
    );
    assert_eq!(
        hook_calls(&kept, &[1]),
        (vec![vec![1], vec![4], vec![4], vec![3]], None)
    );
}

#[test]
fn a_jump_lands_on_its_point_through_the_code_other_patches_put_before_it() {
    const OWNER: &str = "Lcom/example/SkipHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("branch_entry")]);
    apply_fixture(&mut apk, "jump-to-point", "jumpToPoint");

    let code = patched_code(&apk, OWNER, "joinedBefore");
    assert_eq!(
        hook_calls(&code, &[0]),
        (vec![vec![1], vec![2], vec![8], vec![9], vec![3]], None)
    );
    assert_eq!(
        hook_calls(&code, &[1]),
        (vec![vec![1], vec![8], vec![9], vec![3]], None)
    );
}

#[test]
fn hooks_after_an_instruction_run_in_the_order_patches_add_them() {
    const OWNER: &str = "Lcom/example/SkipHost;";

    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("branch_entry")]);
    apply_fixture(&mut apk, "after-order", "afterOrder");

    let code = patched_code(&apk, OWNER, "joinedBefore");
    assert_eq!(
        hook_calls(&code, &[0]),
        (vec![vec![1], vec![2], vec![5], vec![6], vec![3]], None)
    );
}

#[test]
fn a_jump_source_is_the_one_branch_into_an_instruction() {
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("branch_entry")]);
    apply_fixture(&mut apk, "jump-sources", "jumpSources");

    let report = entry(&mut apk, 0, "assets/jump-sources.txt").unwrap();
    assert_eq!(String::from_utf8(report).unwrap(), "3\ntrue");
}
