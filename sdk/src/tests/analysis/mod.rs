// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::analysis::{
    CallStop, ClassRef, HintSource, Member, MethodRef, RefKind, RefTarget, Relation,
};

use crate::analysis::{Analysis, diff};
use crate::fixtures::app;

fn open() -> (tempfile::TempPath, Analysis) {
    let path = app();
    let analysis = Analysis::open(&path, &[]).expect("fixture opens");
    (path, analysis)
}

fn method(text: &str) -> MethodRef {
    text.parse().expect("method descriptor")
}

fn class(text: &str) -> ClassRef {
    text.parse().expect("class descriptor")
}

#[test]
fn the_overview_names_the_entry_points_and_the_app_code() {
    let (_path, mut analysis) = open();

    let overview = analysis.overview().expect("overview");

    assert_eq!(overview.apk.package.as_deref(), Some("com.example.test"));
    assert_eq!(overview.apk.version_code, Some(7));
    assert!(overview.runtimes.is_empty());
    assert_eq!(
        overview.entry.application,
        Some(class("Lcom/example/test/App;"))
    );
    assert_eq!(
        overview.entry.launcher,
        Some(class("Lcom/example/test/MainActivity;"))
    );
    assert_eq!(overview.entry.activities, 1);
    let groups: Vec<(String, usize, usize)> = overview
        .code
        .iter()
        .map(|group| {
            (
                serde_json::to_string(&group.group).expect("json"),
                group.classes,
                group.obfuscated,
            )
        })
        .collect();
    assert_eq!(
        groups,
        [
            (
                r#"{"kind":"app","package":"com.example.test"}"#.to_owned(),
                1,
                0
            ),
            (r#"{"kind":"short_packages"}"#.to_owned(), 26, 26),
        ]
    );
}

#[test]
fn field_references_split_reads_from_writes() {
    let (_path, mut analysis) = open();

    let refs = analysis
        .refs(&RefTarget::Member("Lb7/r;->e:Z".parse().expect("field")))
        .expect("refs");

    let sites: Vec<(String, u32, RefKind)> = refs
        .sites
        .iter()
        .map(|site| (site.site.method.to_string(), site.site.index, site.kind))
        .collect();
    assert_eq!(
        sites,
        [
            ("Lb7/r;-><init>()V".to_owned(), 2, RefKind::Put),
            ("Lb7/r;->c()Z".to_owned(), 0, RefKind::Get),
        ]
    );
    assert_eq!(
        refs.sites[1].site.excerpt,
        "iget-boolean v0, p0, Lb7/r;->e:Z"
    );
}

#[test]
fn method_references_reach_calls_through_the_interface_it_implements() {
    let (_path, mut analysis) = open();

    let refs = analysis
        .refs(&RefTarget::Member(Member::Method(method("Lb7/r;->c()Z"))))
        .expect("refs");

    let sites: Vec<(String, Option<String>)> = refs
        .sites
        .iter()
        .map(|site| {
            (
                site.site.method.to_string(),
                site.target.as_ref().map(ToString::to_string),
            )
        })
        .collect();
    assert_eq!(
        sites,
        [
            (
                "Lb7/s;->a(Lb7/q;)Z".to_owned(),
                Some("Lb7/q;->c()Z".to_owned())
            ),
            (
                "Lcom/example/test/MainActivity;->onCreate(Landroid/os/Bundle;)V".to_owned(),
                Some("Lb7/r;->c()Z".to_owned())
            ),
        ]
    );
    assert_eq!(
        refs.relations,
        [Relation::Overridden {
            method: method("Lb7/q;->c()Z")
        }]
    );
}

#[test]
fn callers_stop_where_the_system_calls_in() {
    let (_path, mut analysis) = open();

    let tree = analysis
        .callers(&method("Lb7/r;->c()Z"), 3)
        .expect("callers");

    let activity = tree
        .nodes
        .iter()
        .find(|node| node.site.method.name == "onCreate")
        .expect("the activity calls it");
    assert_eq!(
        activity.stop,
        Some(CallStop::NoCallers {
            external_supertypes: vec![class("Landroid/app/Activity;")]
        })
    );
}

#[test]
fn text_leads_to_the_resource_its_layout_and_the_code_loading_it() {
    let (_path, mut analysis) = open();

    let matches = analysis.find_text("upgrade TO").expect("text");

    let [resource] = matches.resources.as_slice() else {
        panic!("one resource: {matches:?}");
    };
    assert_eq!(resource.resource.name.as_deref(), Some("@string/upgrade"));
    assert_eq!(resource.value, "Upgrade to Pro");
    assert_eq!(resource.loaded_by[0].excerpt, "const v0, 0x7f010000");
    let [hit] = resource.xml.as_slice() else {
        panic!("one layout: {resource:?}");
    };
    assert_eq!(hit.usage.file, "res/layout/paywall.xml");
    assert_eq!(hit.usage.element, "TextView");
    assert_eq!(
        hit.usage
            .resource
            .as_ref()
            .and_then(|id| id.name.as_deref()),
        Some("@layout/paywall")
    );
    assert_eq!(
        hit.usage.view_id.as_ref().and_then(|id| id.name.as_deref()),
        Some("@id/title")
    );
}

#[test]
fn methods_show_as_smali_with_the_names_their_class_still_holds() {
    let (_path, analysis) = open();

    let view = analysis.show_method(&method("Lb7/r;->c()Z")).expect("show");

    let lines: Vec<&str> = view.lines.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(lines, ["iget-boolean v0, p0, Lb7/r;->e:Z", "return v0"]);
    assert_eq!(
        view.lines[0].refers,
        Some("Lb7/r;->e:Z".parse().expect("field"))
    );
    assert!(
        view.hints
            .iter()
            .any(|hint| hint.name == "EntitlementRepository" && hint.source == HintSource::ToString)
    );
}

#[test]
fn a_diff_aligns_two_bodies_on_what_they_share() {
    let (_path, analysis) = open();
    let getter = analysis.show_method(&method("Lb7/r;->c()Z")).expect("show");
    let constructor = analysis
        .show_method(&method("Lb7/r;-><init>()V"))
        .expect("show");

    let same = diff(&getter, &getter);
    let changed = diff(&getter, &constructor);

    assert_eq!((same.added, same.removed), (0, 0));
    assert_eq!((changed.added, changed.removed), (4, 2));
}

#[test]
fn a_name_the_dex_still_holds_finds_its_class() {
    let (_path, analysis) = open();
    let named = analysis.named("EntitlementRepository").expect("a lookup");
    assert_eq!(named.len(), 1);
    assert_eq!(named[0].member, Member::Class(class("Lb7/r;")));
    assert_eq!(named[0].hint.source, HintSource::ToString);
    assert!(analysis.named("NoSuchName").expect("a lookup").is_empty());
}

#[test]
fn calls_through_an_interface_many_classes_implement_are_left_out() {
    let (_path, mut analysis) = open();
    let lambda = method("Lb8/l3;->invoke()V");
    let references = analysis
        .refs(&RefTarget::Member(Member::Method(lambda.clone())))
        .expect("references");
    assert!(references.sites.is_empty());
    assert_eq!(
        references.relations,
        [Relation::Shared {
            method: method("Lb8/f;->invoke()V"),
            implementers: 21,
        }]
    );
    let tree = analysis.callers(&lambda, 3).expect("callers");
    assert!(tree.nodes.is_empty());
    assert_eq!(
        tree.stop,
        Some(CallStop::Shared {
            through: method("Lb8/f;->invoke()V"),
            implementers: 21,
        })
    );
}
