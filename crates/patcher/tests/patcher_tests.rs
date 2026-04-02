use stitch_apk::ApkFile;
use stitch_apk::stitch_dex::ParseOptions;
use stitch_patcher::context::PatchContext;
use stitch_patcher::engine::{self, ExecutionPlan, PatchStatus};
use stitch_patcher::error::Result as PatcherResult;
use stitch_patcher::options::{OptionDeclaration, OptionType, OptionValue, PatchOptions};
use stitch_patcher::patch::{Compatibility, Patch};

const YOUTUBE_APK: &str =
    "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";

fn has_apk() -> bool {
    std::path::Path::new(YOUTUBE_APK).exists()
}

fn open_test_apk() -> ApkFile {
    ApkFile::open_with_options(
        YOUTUBE_APK,
        ParseOptions {
            lazy: true,
            ..ParseOptions::default()
        },
    )
    .expect("open failed")
}

struct StubPatch {
    name: String,
    compat: Vec<Compatibility>,
    enabled: bool,
    deps: Vec<String>,
    options: Vec<OptionDeclaration>,
    action: Box<dyn Fn(&mut PatchContext) -> PatcherResult<()> + Send + Sync>,
}

impl StubPatch {
    fn noop(name: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            compat: vec![],
            enabled: true,
            deps: vec![],
            options: vec![],
            action: Box::new(|_| Ok(())),
        })
    }

    fn for_package(name: &str, pkg: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            compat: vec![Compatibility::package(pkg)],
            enabled: true,
            deps: vec![],
            options: vec![],
            action: Box::new(|_| Ok(())),
        })
    }

    fn for_version(name: &str, ver: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            compat: vec![Compatibility::with_versions(
                "com.google.android.youtube",
                vec![ver.to_string()],
            )],
            enabled: true,
            deps: vec![],
            options: vec![],
            action: Box::new(|_| Ok(())),
        })
    }

    fn failing(name: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            compat: vec![],
            enabled: true,
            deps: vec![],
            options: vec![],
            action: Box::new(|_| {
                Err(stitch_patcher::error::PatcherError::PatchFailed {
                    name: "test".into(),
                    reason: "intentional failure".into(),
                })
            }),
        })
    }

    fn disabled(name: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            compat: vec![],
            enabled: false,
            deps: vec![],
            options: vec![],
            action: Box::new(|_| Ok(())),
        })
    }

    fn depends_on(name: &str, dependencies: &[&str]) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            compat: vec![],
            enabled: true,
            deps: dependencies.iter().map(|d| (*d).to_string()).collect(),
            options: vec![],
            action: Box::new(|_| Ok(())),
        })
    }

    fn with_required_option(name: &str, option_key: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            compat: vec![],
            enabled: true,
            deps: vec![],
            options: vec![OptionDeclaration {
                key: option_key.to_string(),
                title: "Test Option".to_string(),
                description: "test".to_string(),
                option_type: OptionType::String,
                default_value: None,
                valid_values: None,
                required: true,
            }],
            action: Box::new(|_| Ok(())),
        })
    }

    fn with_bool_option(name: &str, option_key: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            compat: vec![],
            enabled: true,
            deps: vec![],
            options: vec![OptionDeclaration {
                key: option_key.to_string(),
                title: "Toggle".to_string(),
                description: "test".to_string(),
                option_type: OptionType::Bool,
                default_value: Some(OptionValue::Bool(false)),
                valid_values: None,
                required: false,
            }],
            action: Box::new(|_| Ok(())),
        })
    }
}

impl Patch for StubPatch {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        "test patch"
    }
    fn compatible_with(&self) -> &[Compatibility] {
        &self.compat
    }
    fn enabled_by_default(&self) -> bool {
        self.enabled
    }
    fn depends_on(&self) -> &[String] {
        &self.deps
    }
    fn options(&self) -> &[OptionDeclaration] {
        &self.options
    }
    fn execute(&self, ctx: &mut PatchContext) -> PatcherResult<()> {
        (self.action)(ctx)
    }
}

#[test]
fn test_engine_compatibility() {
    if !has_apk() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let mut apk = open_test_apk();
    let pkg = apk.package_name().expect("no package").to_owned();
    let ver = apk.version_name().expect("no version").to_owned();

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![StubPatch::noop("universal")];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, PatchStatus::Applied);
        assert_eq!(results[0].name, "universal");
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> =
            vec![StubPatch::for_package("wrong-pkg", "com.example.wrong")];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        match &results[0].status {
            PatchStatus::Skipped { reason } => {
                assert_eq!(results[0].name, "wrong-pkg");
                assert!(reason.contains("incompatible package"), "got: {reason}");
            }
            other => panic!("expected Skipped, got: {other:?}"),
        }
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![StubPatch::for_package("right-pkg", &pkg)];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert_eq!(results[0].status, PatchStatus::Applied);
        assert_eq!(results[0].name, "right-pkg");
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> =
            vec![StubPatch::for_version("wrong-ver", "0.0.0-nonexistent")];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        match &results[0].status {
            PatchStatus::Skipped { reason } => {
                assert_eq!(results[0].name, "wrong-ver");
                assert!(reason.contains("incompatible version"), "got: {reason}");
            }
            other => panic!("expected Skipped, got: {other:?}"),
        }
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![StubPatch::for_version("right-ver", &ver)];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert_eq!(results[0].status, PatchStatus::Applied);
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![
            StubPatch::noop("first"),
            StubPatch::for_package("wrong", "com.wrong"),
            StubPatch::for_package("right", &pkg),
            StubPatch::noop("last"),
        ];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].status, PatchStatus::Applied);
        assert!(matches!(results[1].status, PatchStatus::Skipped { .. }));
        assert_eq!(results[2].status, PatchStatus::Applied);
        assert_eq!(results[3].status, PatchStatus::Applied);
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![
            StubPatch::noop("before"),
            StubPatch::failing("bad"),
            StubPatch::noop("after"),
        ];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].status, PatchStatus::Applied);
        assert!(matches!(results[1].status, PatchStatus::Failed { .. }));
        assert_eq!(results[2].status, PatchStatus::Applied);
        if let PatchStatus::Failed { reason } = &results[1].status {
            assert!(reason.contains("intentional failure"), "got: {reason}");
        }
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![StubPatch::disabled("disabled")];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert!(matches!(results[0].status, PatchStatus::Skipped { .. }));
        if let PatchStatus::Skipped { reason } = &results[0].status {
            assert!(reason.contains("not selected"), "got: {reason}");
        }
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![
            StubPatch::disabled("base"),
            StubPatch::depends_on("dependent", &["base"]),
        ];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert_eq!(results[0].status, PatchStatus::Applied);
        assert_eq!(results[1].status, PatchStatus::Applied);
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert!(results.is_empty());
    }

    eprintln!("engine compatibility tests OK");
}

#[test]
fn test_execution_plan_selects_only_requested_patches() {
    if !has_apk() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let mut apk = open_test_apk();
    let mut ctx = PatchContext::new(&mut apk);
    let patches: Vec<Box<dyn Patch>> = vec![StubPatch::noop("alpha"), StubPatch::noop("beta")];
    let mut plan = ExecutionPlan::new();
    plan.select_patch("beta");

    let results = engine::apply_patches_with_plan(&mut ctx, &patches, &plan).expect("apply failed");
    assert!(matches!(results[0].status, PatchStatus::Skipped { .. }));
    assert_eq!(results[1].status, PatchStatus::Applied);
}

#[test]
fn test_execution_plan_disables_selected_patch() {
    if !has_apk() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let mut apk = open_test_apk();
    let mut ctx = PatchContext::new(&mut apk);
    let patches: Vec<Box<dyn Patch>> = vec![StubPatch::noop("alpha")];
    let mut plan = ExecutionPlan::new();
    plan.select_patch("alpha");
    plan.disable_patch("alpha");

    let err = engine::apply_patches_with_plan(&mut ctx, &patches, &plan).expect_err("should fail");
    assert!(err.to_string().contains("both selected and disabled"));
}

#[test]
fn test_missing_required_option_is_reported() {
    if !has_apk() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let mut apk = open_test_apk();
    let mut ctx = PatchContext::new(&mut apk);
    let patches: Vec<Box<dyn Patch>> = vec![StubPatch::with_required_option("alpha", "token")];

    let err = engine::apply_patches(&mut ctx, &patches).expect_err("should fail");
    assert!(err.to_string().contains("missing required option"));
}

#[test]
fn test_option_type_validation_is_reported() {
    if !has_apk() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let mut apk = open_test_apk();
    let mut ctx = PatchContext::new(&mut apk);
    let patches: Vec<Box<dyn Patch>> = vec![StubPatch::with_bool_option("alpha", "toggle")];
    let mut plan = ExecutionPlan::new();
    let mut options = PatchOptions::new();
    options.set("toggle", OptionValue::String("not-bool".to_string()));
    plan.set_patch_options("alpha", options);

    let err = engine::apply_patches_with_plan(&mut ctx, &patches, &plan).expect_err("should fail");
    assert!(err.to_string().contains("invalid option value"));
}

#[test]
fn test_disabled_patch_configuration_is_rejected() {
    if !has_apk() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let mut apk = open_test_apk();
    let mut ctx = PatchContext::new(&mut apk);
    let patches: Vec<Box<dyn Patch>> = vec![StubPatch::with_bool_option("alpha", "toggle")];
    let mut plan = ExecutionPlan::new();
    plan.disable_patch("alpha");
    let mut options = PatchOptions::new();
    options.set("toggle", OptionValue::Bool(true));
    plan.set_patch_options("alpha", options);

    let err = engine::apply_patches_with_plan(&mut ctx, &patches, &plan).expect_err("should fail");
    assert!(err.to_string().contains("options configured but is not enabled"));
}

#[test]
fn test_bundle_load_missing_dir() {
    let result = stitch_patcher::bundle::PatchBundle::load("/nonexistent/path");
    assert!(result.is_err());
}

#[test]
fn test_missing_dependency_is_reported() {
    let patches: Vec<Box<dyn Patch>> = vec![StubPatch::depends_on("missing", &["ghost"])];
    let err = stitch_patcher::dependency::sort_patches(&patches).expect_err("should fail");
    let msg = err.to_string();
    assert!(msg.contains("missing dependency"), "got: {msg}");
    assert!(msg.contains("ghost"), "got: {msg}");
}

#[test]
fn test_bundle_load_minimal() {
    let tmp = tempfile::tempdir().expect("tempdir failed");
    let bundle_toml = tmp.path().join("bundle.toml");
    std::fs::write(
        &bundle_toml,
        r#"
[bundle]
name = "test-bundle"
"#,
    )
    .expect("write failed");

    let bundle =
        stitch_patcher::bundle::PatchBundle::load(tmp.path()).expect("load failed");
    assert_eq!(bundle.name, "test-bundle");
    assert!(bundle.author.is_empty());
    assert!(bundle.patches.is_empty());
    assert!(bundle.extension_dex.is_empty());
}

#[test]
fn test_bundle_load_full_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir failed");
    let bundle_toml = tmp.path().join("bundle.toml");
    std::fs::write(
        &bundle_toml,
        r#"
[bundle]
name = "my-patches"
author = "tester"
description = "test bundle"
"#,
    )
    .expect("write failed");

    let bundle =
        stitch_patcher::bundle::PatchBundle::load(tmp.path()).expect("load failed");
    assert_eq!(bundle.name, "my-patches");
    assert_eq!(bundle.author, "tester");
    assert_eq!(bundle.description, "test bundle");
}

#[test]
fn test_bundle_load_no_patches() {
    let tmp = tempfile::tempdir().expect("tempdir failed");
    let bundle_toml = tmp.path().join("bundle.toml");
    std::fs::write(
        &bundle_toml,
        r#"
[bundle]
name = "empty"
"#,
    )
    .expect("write failed");

    let bundle =
        stitch_patcher::bundle::PatchBundle::load(tmp.path()).expect("load failed");
    assert!(bundle.patches.is_empty());
}

#[test]
fn test_bundle_load_invalid_toml() {
    let tmp = tempfile::tempdir().expect("tempdir failed");
    let bundle_toml = tmp.path().join("bundle.toml");
    std::fs::write(&bundle_toml, "this is not valid toml {{{").expect("write failed");

    let result = stitch_patcher::bundle::PatchBundle::load(tmp.path());
    assert!(result.is_err());
}

#[test]
fn test_bundle_discovers_extension_dex() {
    let tmp = tempfile::tempdir().expect("tempdir failed");
    std::fs::write(
        tmp.path().join("bundle.toml"),
        r#"
[bundle]
name = "ext-test"
"#,
    )
    .expect("write failed");

    let ext_dir = tmp.path().join("extensions");
    std::fs::create_dir(&ext_dir).expect("mkdir failed");
    std::fs::write(ext_dir.join("ext1.dex"), b"fake").expect("write failed");
    std::fs::write(ext_dir.join("ext2.dex"), b"fake").expect("write failed");
    std::fs::write(ext_dir.join("readme.txt"), b"not a dex").expect("write failed");

    let bundle =
        stitch_patcher::bundle::PatchBundle::load(tmp.path()).expect("load failed");
    assert_eq!(bundle.extension_dex.len(), 2);
    assert!(bundle.extension_dex[0].ends_with("ext1.dex"));
    assert!(bundle.extension_dex[1].ends_with("ext2.dex"));
}

#[test]
fn test_context_with_apk() {
    if !has_apk() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let mut apk = open_test_apk();

    {
        let ctx = PatchContext::new(&mut apk);
        assert!(ctx.package_name().is_some());
        assert!(ctx.version_name().is_some());
        assert!(ctx.version_code().is_some());
        assert!(ctx.dex_count() > 0);
        assert!(ctx.dex_file(0).is_some());
        assert!(ctx.dex_file(999).is_none());
        assert!(ctx.manifest().package_name().is_some());
    }

    {
        let ctx = PatchContext::new(&mut apk);
        let dex = ctx.dex_file(0).expect("dex 0");
        let first_class_type = dex.type_descriptor(dex.classes[0].class_type).to_owned();
        assert!(ctx.find_class(&first_class_type).is_some());
        assert!(ctx.find_class("Lcom/nonexistent/Class;").is_none());
    }

    {
        let mut ctx = PatchContext::new(&mut apk);
        ctx.dex_mut(0)
            .expect("dex 0")
            .resolve_all_class_data()
            .expect("resolve class data");
        let dex = ctx.dex_file(0).expect("dex 0");
        let mut found_class = None;
        let mut found_method = None;
        for class in &dex.classes {
            if let Some(ref data) = class.class_data {
                let methods = data.direct_methods.iter().chain(data.virtual_methods.iter());
                for m in methods {
                    if m.code.is_some() {
                        found_class = Some(dex.type_descriptor(class.class_type).to_owned());
                        let method_id = &dex.methods[m.method.0 as usize];
                        found_method = Some(dex.strings[method_id.name.0 as usize].as_str().to_owned());
                        break;
                    }
                }
            }
            if found_class.is_some() {
                break;
            }
        }
        let class_desc = found_class.expect("should find a class with methods");
        let method_name = found_method.expect("should find a method");
        assert!(ctx.find_method_mut(&class_desc, &method_name).is_some());
        assert!(ctx.find_method_mut("Lcom/nonexistent/Class;", "missing").is_none());
    }

    eprintln!("context tests OK");
}
