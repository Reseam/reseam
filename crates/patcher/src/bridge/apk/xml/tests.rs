// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::attributes::{xml_get_attribute, xml_set_attribute};
use super::documents::{xml_close, xml_open};
use super::tree::{
    CloneDepth, xml_adopt, xml_append_child, xml_clone_element, xml_create_element,
    xml_insert_before, xml_remove_element,
};
use super::*;
use crate::bridge::apk::{manifest, manifest_activities};
use crate::bridge::phase::{ContextGuard, RunningBundle};
use crate::bridge::run_state::{RunGuard, check_invocation};
use crate::context::{ExtensionSet, PatchContext};
use reseam_apk::Compression;
use reseam_apk::axml::AxmlDocument;

#[test]
fn manifest_views_share_borrows_and_keep_element_identities() {
    let (dir, mut apk) = crate::test_support::apk();
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut ctx, RunningBundle::at(dir.path().to_owned())).unwrap();
    let outer = manifest::manifest_get_document(None).unwrap();
    let application = xml_find_by_tag(outer, "application".into())[0];
    let main = xml_find_by_attribute(outer, "android:name".into(), ".Main".into())[0];
    let nested = xml_open(None, "AndroidManifest.xml".into()).unwrap();
    let root = xml_root(nested);
    xml_set_attribute(nested, root, "android:versionName".into(), "3.0".into()).unwrap();
    assert_eq!(
        manifest::manifest_version_name(None).as_deref(),
        Some("3.0")
    );
    xml_close(nested);
    manifest::manifest_set_version_name(None, "4.0".into());
    assert_eq!(
        xml_get_attribute(outer, root, "android:versionName".into()).as_deref(),
        Some("4.0")
    );
    manifest::manifest_add_permission(None, "android.permission.INTERNET".into());
    manifest_activities::manifest_set_activity_config_changes(
        None,
        ".Main".into(),
        "orientation|screenSize|unknown".into(),
    );
    assert_eq!(
        xml_get_attribute(outer, main, "android:configChanges".into()).as_deref(),
        Some("1152")
    );
    manifest_activities::manifest_add_intent_filter(
        None,
        ".Main".into(),
        Some("android.intent.action.VIEW".into()),
        None,
        None,
    );
    manifest_activities::manifest_add_activity_alias(
        None,
        ".Main".into(),
        ".Alias".into(),
        false,
        Some("alias".into()),
    );
    manifest_activities::manifest_copy_intent_filters(None, ".Main".into(), ".Alias".into());
    xml_declare_namespace(outer, "test".into(), "urn:test".into()).unwrap();
    assert_eq!(xml_tag_name(outer, application), "application");
    assert_eq!(
        xml_get_attribute(outer, main, "android:name".into()).as_deref(),
        Some(".Main")
    );
    assert_eq!(xml_parent(outer, main), Some(application));
    let alias = xml_find_by_attribute(outer, "android:name".into(), ".Alias".into())[0];
    assert_eq!(
        xml_children(outer, alias)
            .iter()
            .map(|child| xml_tag_name(outer, *child))
            .collect::<Vec<_>>(),
        ["intent-filter"]
    );
    xml_close(outer);
    guard.finish().unwrap();
    assert_eq!(apk.base().manifest().version_name().as_deref(), Some("4.0"));
}

#[test]
fn subtree_handles_follow_moves_and_removal_without_consuming_children() {
    let (dir, mut apk) = crate::test_support::apk();
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    let guard = ContextGuard::enter(&mut ctx, RunningBundle::at(dir.path().to_owned())).unwrap();
    let document = manifest::manifest_get_document(None).unwrap();
    let application = xml_find_by_tag(document, "application".into())[0];
    let parent = xml_create_element(document, "activity".into());
    let filter = xml_create_element(document, "intent-filter".into());
    let action = xml_create_element(document, "action".into());
    xml_set_attribute(
        document,
        action,
        "android:name".into(),
        "test.action".into(),
    )
    .unwrap();
    xml_append_child(document, filter, action).unwrap();
    xml_append_child(document, parent, filter).unwrap();
    xml_append_child(document, application, parent).unwrap();
    assert_eq!(xml_parent(document, action), Some(filter));
    assert_eq!(xml_parent(document, filter), Some(parent));
    let first = xml_children(document, application)[0];
    assert_eq!(xml_insert_before(document, parent, first).unwrap(), parent);
    assert_eq!(xml_children(document, application)[0], parent);
    assert_eq!(
        xml_get_attribute(document, action, "android:name".into()).as_deref(),
        Some("test.action")
    );
    let cloned = xml_clone_element(document, parent, CloneDepth::Shallow);
    assert!(xml_children(document, cloned).is_empty());
    xml_append_child(document, application, cloned).unwrap();
    xml_remove_element(document, parent);
    assert_eq!(xml_tag_name(document, first), "activity");
    assert_eq!(xml_tag_name(document, cloned), "activity");
    assert!(!xml_children(document, application).contains(&parent));
    xml_close(document);
    check_invocation().unwrap();
    guard.finish().unwrap();
}

#[test]
fn file_edits_commit_on_last_close_and_unfinished_edits_fail() {
    for close in [true, false] {
        let (dir, mut apk) = crate::test_support::apk();
        let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
        ctx.inject_file(0, "res/layout/main.xml", br#"<LinearLayout xmlns:android="http://schemas.android.com/apk/res/android"><TextView android:text="original"/></LinearLayout>"#.to_vec(), Compression::Deflated).unwrap();
        let _run = RunGuard::enter().unwrap();
        let guard =
            ContextGuard::enter(&mut ctx, RunningBundle::at(dir.path().to_owned())).unwrap();
        let outer = xml_open(None, "res/layout/main.xml".into()).unwrap();
        let nested = xml_open(None, "res/layout/main.xml".into()).unwrap();
        let label = xml_find_by_tag(outer, "TextView".into())[0];
        let source = xml_compile(r#"<TextView xmlns:android="http://schemas.android.com/apk/res/android" android:text="adopted"/>"#.into()).unwrap();
        let adopted = xml_adopt(outer, source, xml_root(source)).unwrap();
        xml_append_child(outer, xml_root(outer), adopted).unwrap();
        xml_close(source);
        assert_eq!(
            xml_get_attribute(outer, adopted, "android:text".into()).as_deref(),
            Some("adopted")
        );
        xml_set_attribute(outer, label, "android:text".into(), "updated".into()).unwrap();
        xml_close(nested);
        assert_eq!(
            xml_get_attribute(outer, label, "android:text".into()).as_deref(),
            Some("updated")
        );
        if close {
            xml_close(outer);
        }
        assert_eq!(check_invocation().is_ok(), close);
        assert_eq!(guard.finish().is_ok(), close);
        let bytes = ctx.read_file(0, "res/layout/main.xml").unwrap().unwrap();
        let written = AxmlDocument::parse(&bytes).unwrap();
        let label = written.find_element("TextView").unwrap();
        assert_eq!(
            written
                .attribute_named(label, "text")
                .and_then(|value| written.attribute_string(value))
                .as_deref(),
            Some(if close { "updated" } else { "original" })
        );
    }
}
