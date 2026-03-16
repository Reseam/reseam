use stitch_apk::ApkFile;
use stitch_patcher::context::PatchContext;
use stitch_patcher::engine::{self, PatchResult};
use stitch_patcher::error::Result as PatcherResult;
use stitch_patcher::patch::Patch;

const YOUTUBE_APK: &str =
    "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";

fn has_apk() -> bool {
    std::path::Path::new(YOUTUBE_APK).exists()
}

// --- Stub patches for engine tests ---

struct StubPatch {
    name: String,
    packages: Vec<String>,
    versions: Vec<String>,
    enabled: bool,
    action: Box<dyn Fn(&mut PatchContext) -> PatcherResult<()> + Send + Sync>,
}

impl StubPatch {
    fn noop(name: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            packages: vec![],
            versions: vec![],
            enabled: true,
            action: Box::new(|_| Ok(())),
        })
    }

    fn for_package(name: &str, pkg: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            packages: vec![pkg.to_string()],
            versions: vec![],
            enabled: true,
            action: Box::new(|_| Ok(())),
        })
    }

    fn for_version(name: &str, ver: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            packages: vec![],
            versions: vec![ver.to_string()],
            enabled: true,
            action: Box::new(|_| Ok(())),
        })
    }

    fn failing(name: &str) -> Box<dyn Patch> {
        Box::new(Self {
            name: name.to_string(),
            packages: vec![],
            versions: vec![],
            enabled: true,
            action: Box::new(|_| {
                Err(stitch_patcher::error::PatcherError::PatchFailed {
                    name: "test".into(),
                    reason: "intentional failure".into(),
                })
            }),
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
    fn compatible_packages(&self) -> &[String] {
        &self.packages
    }
    fn compatible_versions(&self) -> &[String] {
        &self.versions
    }
    fn enabled_by_default(&self) -> bool {
        self.enabled
    }
    fn execute(&self, ctx: &mut PatchContext) -> PatcherResult<()> {
        (self.action)(ctx)
    }
}

// --- Engine compatibility tests (single APK open) ---

#[test]
fn test_engine_compatibility() {
    if !has_apk() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let mut apk = ApkFile::open(YOUTUBE_APK).expect("open failed");
    let pkg = apk.package_name().expect("no package").to_owned();
    let ver = apk.version_name().expect("no version").to_owned();

    // universal patch applies
    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![StubPatch::noop("universal")];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0],
            PatchResult::Applied {
                name: "universal".into()
            }
        );
    }

    // incompatible package is skipped
    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> =
            vec![StubPatch::for_package("wrong-pkg", "com.example.wrong")];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        match &results[0] {
            PatchResult::Skipped { name, reason } => {
                assert_eq!(name, "wrong-pkg");
                assert!(reason.contains("incompatible package"), "got: {reason}");
            }
            other => panic!("expected Skipped, got: {other:?}"),
        }
    }

    // matching package applies
    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![StubPatch::for_package("right-pkg", &pkg)];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert_eq!(
            results[0],
            PatchResult::Applied {
                name: "right-pkg".into()
            }
        );
    }

    // incompatible version is skipped
    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> =
            vec![StubPatch::for_version("wrong-ver", "0.0.0-nonexistent")];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        match &results[0] {
            PatchResult::Skipped { name, reason } => {
                assert_eq!(name, "wrong-ver");
                assert!(reason.contains("incompatible version"), "got: {reason}");
            }
            other => panic!("expected Skipped, got: {other:?}"),
        }
    }

    // matching version applies
    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![StubPatch::for_version("right-ver", &ver)];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert_eq!(
            results[0],
            PatchResult::Applied {
                name: "right-ver".into()
            }
        );
    }

    // mixed compatible and incompatible
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
        assert!(matches!(&results[0], PatchResult::Applied { .. }));
        assert!(matches!(&results[1], PatchResult::Skipped { .. }));
        assert!(matches!(&results[2], PatchResult::Applied { .. }));
        assert!(matches!(&results[3], PatchResult::Applied { .. }));
    }

    // failing patch returns error
    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![
            StubPatch::noop("before"),
            StubPatch::failing("bad"),
            StubPatch::noop("after"),
        ];
        let err = engine::apply_patches(&mut ctx, &patches).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("bad"), "error should name the patch: {msg}");
    }

    // empty patches
    {
        let mut ctx = PatchContext::new(&mut apk);
        let patches: Vec<Box<dyn Patch>> = vec![];
        let results = engine::apply_patches(&mut ctx, &patches).expect("apply failed");
        assert!(results.is_empty());
    }

    eprintln!("engine compatibility tests OK");
}

// --- Bundle loading tests (no APK needed) ---

#[test]
fn test_bundle_load_missing_dir() {
    let result = stitch_patcher::bundle::PatchBundle::load("/nonexistent/path");
    assert!(result.is_err());
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
fn test_bundle_load_missing_lua_script() {
    let tmp = tempfile::tempdir().expect("tempdir failed");
    let bundle_toml = tmp.path().join("bundle.toml");
    std::fs::write(
        &bundle_toml,
        r#"
patches = ["nonexistent.lua"]

[bundle]
name = "broken"
"#,
    )
    .expect("write failed");

    let err = stitch_patcher::bundle::PatchBundle::load(tmp.path())
        .err()
        .expect("should have failed");
    let msg = err.to_string();
    assert!(msg.contains("not found"), "got: {msg}");
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

// --- Lua patch tests ---

#[cfg(feature = "lua")]
mod lua_tests {
    use super::*;

    fn write_lua_patch(dir: &std::path::Path, filename: &str, source: &str) {
        std::fs::write(dir.join(filename), source).expect("write lua failed");
    }

    #[test]
    fn test_lua_patch_loading() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        write_lua_patch(
            tmp.path(),
            "test.lua",
            r#"
return {
    name = "lua-test",
    description = "a test patch",
    compatible_packages = {"com.example.app"},
    compatible_versions = {"1.0", "2.0"},
    enabled_by_default = false,
    execute = function(ctx) end
}
"#,
        );

        std::fs::write(
            tmp.path().join("bundle.toml"),
            r#"
patches = ["test.lua"]

[bundle]
name = "lua-bundle"
"#,
        )
        .expect("write toml failed");

        let bundle =
            stitch_patcher::bundle::PatchBundle::load(tmp.path()).expect("load failed");
        assert_eq!(bundle.patches.len(), 1);

        let p = bundle.patches[0].as_ref();
        assert_eq!(p.name(), "lua-test");
        assert_eq!(p.description(), "a test patch");
        assert_eq!(p.compatible_packages(), &["com.example.app"]);
        assert_eq!(p.compatible_versions(), &["1.0", "2.0"]);
        assert!(!p.enabled_by_default());
    }

    #[test]
    fn test_lua_patch_missing_name() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        write_lua_patch(
            tmp.path(),
            "bad.lua",
            r#"
return {
    execute = function(ctx) end
}
"#,
        );

        std::fs::write(
            tmp.path().join("bundle.toml"),
            r#"
patches = ["bad.lua"]

[bundle]
name = "bad-bundle"
"#,
        )
        .expect("write toml failed");

        let err = stitch_patcher::bundle::PatchBundle::load(tmp.path())
            .err()
            .expect("should have failed");
        let msg = err.to_string();
        assert!(msg.contains("name"), "got: {msg}");
    }

    #[test]
    fn test_lua_patch_missing_execute() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        write_lua_patch(
            tmp.path(),
            "no_exec.lua",
            r#"
return {
    name = "no-exec"
}
"#,
        );

        std::fs::write(
            tmp.path().join("bundle.toml"),
            r#"
patches = ["no_exec.lua"]

[bundle]
name = "no-exec-bundle"
"#,
        )
        .expect("write toml failed");

        let err = stitch_patcher::bundle::PatchBundle::load(tmp.path())
            .err()
            .expect("should have failed");
        let msg = err.to_string();
        assert!(msg.contains("execute"), "got: {msg}");
    }

    #[test]
    fn test_lua_patch_syntax_error() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        write_lua_patch(tmp.path(), "syntax.lua", "this is not valid lua {{{{");

        std::fs::write(
            tmp.path().join("bundle.toml"),
            r#"
patches = ["syntax.lua"]

[bundle]
name = "syntax-bundle"
"#,
        )
        .expect("write toml failed");

        let result = stitch_patcher::bundle::PatchBundle::load(tmp.path());
        assert!(result.is_err());
    }

    // All APK-dependent Lua tests combined into one
    #[test]
    fn test_lua_patches_with_apk() {
        if !has_apk() {
            eprintln!("Skipping: APK not found");
            return;
        }

        let mut apk = ApkFile::open(YOUTUBE_APK).expect("open failed");

        // --- execution test ---
        {
            let tmp = tempfile::tempdir().expect("tempdir failed");
            write_lua_patch(
                tmp.path(),
                "exec.lua",
                r#"
return {
    name = "exec-test",
    execute = function(ctx)
        local pkg = ctx:package_name()
        if not pkg then
            error("no package name")
        end
        local count = ctx:dex_count()
        if count < 1 then
            error("no dex files")
        end
    end
}
"#,
            );

            std::fs::write(
                tmp.path().join("bundle.toml"),
                r#"
patches = ["exec.lua"]

[bundle]
name = "exec-bundle"
"#,
            )
            .expect("write toml failed");

            let bundle =
                stitch_patcher::bundle::PatchBundle::load(tmp.path()).expect("load failed");
            let mut ctx = PatchContext::new(&mut apk);
            let results =
                engine::apply_patches(&mut ctx, &bundle.patches).expect("apply failed");
            assert_eq!(results.len(), 1);
            assert!(matches!(&results[0], PatchResult::Applied { .. }));
        }

        // --- find_class test: find first real class from dex[0] ---
        let first_class = {
            let ctx = PatchContext::new(&mut apk);
            let dex = ctx.dex_file(0).expect("dex 0");
            dex.type_descriptor(dex.classes[0].class_type).to_owned()
        };
        {
            let tmp = tempfile::tempdir().expect("tempdir failed");
            write_lua_patch(
                tmp.path(),
                "find.lua",
                &format!(r#"
return {{
    name = "find-test",
    execute = function(ctx)
        local result = ctx:find_class("{first_class}")
        if not result then
            error("could not find class")
        end
        if result.dex_index == nil then
            error("missing dex_index")
        end
    end
}}
"#),
            );

            std::fs::write(
                tmp.path().join("bundle.toml"),
                r#"
patches = ["find.lua"]

[bundle]
name = "find-bundle"
"#,
            )
            .expect("write toml failed");

            let bundle =
                stitch_patcher::bundle::PatchBundle::load(tmp.path()).expect("load failed");
            let mut ctx = PatchContext::new(&mut apk);
            let results =
                engine::apply_patches(&mut ctx, &bundle.patches).expect("apply failed");
            assert!(matches!(&results[0], PatchResult::Applied { .. }));
        }

        // --- runtime error test ---
        {
            let tmp = tempfile::tempdir().expect("tempdir failed");
            write_lua_patch(
                tmp.path(),
                "err.lua",
                r#"
return {
    name = "error-test",
    execute = function(ctx)
        error("intentional lua error")
    end
}
"#,
            );

            std::fs::write(
                tmp.path().join("bundle.toml"),
                r#"
patches = ["err.lua"]

[bundle]
name = "err-bundle"
"#,
            )
            .expect("write toml failed");

            let bundle =
                stitch_patcher::bundle::PatchBundle::load(tmp.path()).expect("load failed");
            let mut ctx = PatchContext::new(&mut apk);
            let err = engine::apply_patches(&mut ctx, &bundle.patches).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("intentional lua error"), "got: {msg}");
        }

        eprintln!("lua APK tests OK");
    }
}

// --- PatchContext tests (single APK open) ---

#[test]
fn test_context_with_apk() {
    if !has_apk() {
        eprintln!("Skipping: APK not found");
        return;
    }

    let mut apk = ApkFile::open(YOUTUBE_APK).expect("open failed");

    // accessors
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

    // find_class - use an app-defined class, not a framework class
    {
        let ctx = PatchContext::new(&mut apk);
        let dex = ctx.dex_file(0).expect("dex 0");
        let first_class_type = dex.type_descriptor(dex.classes[0].class_type).to_owned();
        assert!(ctx.find_class(&first_class_type).is_some());
        assert!(ctx.find_class("Lcom/nonexistent/Class;").is_none());
    }

    // find_method_mut - find a real method in the first class with methods
    {
        let mut ctx = PatchContext::new(&mut apk);
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
