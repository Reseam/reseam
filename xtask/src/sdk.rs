// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, ensure, Context, Result};

use crate::jni::Toolchain;
use crate::ndk::{self, AndroidArch, ANDROID_ARCHES};
use crate::paths;
use crate::run::run;

pub fn regen() -> Result<()> {
    let sdk = paths::sdk();
    run(Command::new("boltffi")
        .args(["build", "android", "--release"])
        .current_dir(&sdk))?;
    run(Command::new("boltffi")
        .args(["generate", "kotlin"])
        .current_dir(&sdk))?;
    fix_generated_jni()?;
    run(Command::new("boltffi")
        .args(["generate", "header"])
        .current_dir(&sdk))?;
    let api = ndk::android_api();
    for arch in ANDROID_ARCHES {
        link_android_jni_lib(arch, api)?;
    }
    println!("Regenerated SDK Kotlin bindings and Android jniLibs.");
    Ok(())
}

fn glue_source() -> PathBuf {
    paths::sdk().join("generated/jni/jni_glue.c")
}

fn glue_include() -> PathBuf {
    paths::sdk().join("dist/android/include")
}

const CONTINUATION_LOOKUP: &str = r#"    if (boltffi_lookup_global_class(env, "app/reseam/sdk/Native", &g_callback_class) != BOLTFFI_GLOBAL_CLASS_OK) {
        g_callback_class = NULL;
        return JNI_ERR;
    }
    if (!boltffi_lookup_static_method(env, g_callback_class, "boltffiFutureContinuationCallback", "(JB)V", &g_callback_method)) {
        (*env)->DeleteGlobalRef(env, g_callback_class);
        g_callback_class = NULL;
        g_callback_method = NULL;
        return JNI_ERR;
    }
"#;

const CONTINUATION_LOOKUP_REMOVED: &str = r#"    /*
     * BoltFFI 0.24.1 emits this lookup whenever a callback trait exists, even
     * when the module has only synchronous callbacks. Kotlin correctly omits
     * boltffiFutureContinuationCallback unless async functions/streams/async
     * callbacks exist, so keep JNI_OnLoad limited to the sync callback setup.
     * Remove this once BoltFFI gates the JNI continuation lookup the same way
     * as the Kotlin Native template.
     */
"#;

/// BoltFFI 0.24.1's JNI glue looks up an async continuation callback the
/// generated Kotlin does not define for a synchronous-only module, which
/// makes `JNI_OnLoad` fail. Drops the lookup until BoltFFI gates it.
fn fix_generated_jni() -> Result<()> {
    let jni_path = glue_source();
    let kotlin_path = paths::sdk().join("generated/app/reseam/sdk/ReseamSdk.kt");
    let jni =
        fs::read_to_string(&jni_path).with_context(|| format!("reading {}", jni_path.display()))?;
    let kotlin = fs::read_to_string(&kotlin_path)
        .with_context(|| format!("reading {}", kotlin_path.display()))?;

    if kotlin.contains("fun boltffiFutureContinuationCallback(")
        || jni.contains(CONTINUATION_LOOKUP_REMOVED)
    {
        return Ok(());
    }
    ensure!(
        !jni.contains("_poll(") && !jni.contains("SubscriptionHandle"),
        "generated JNI glue polls async work but the Kotlin side has no continuation callback"
    );
    if !jni.contains(CONTINUATION_LOOKUP) {
        bail!("BoltFFI's JNI continuation lookup changed; update xtask::sdk::fix_generated_jni");
    }
    fs::write(
        &jni_path,
        jni.replace(CONTINUATION_LOOKUP, CONTINUATION_LOOKUP_REMOVED),
    )?;
    println!("Fixed: {}", jni_path.display());
    Ok(())
}

fn link_android_jni_lib(arch: &AndroidArch, api: u32) -> Result<()> {
    let workspace = paths::workspace_root();
    let archive = workspace
        .join("target")
        .join(arch.triple)
        .join("release/libreseam_sdk.a");
    ensure!(
        archive.is_file(),
        "Android static library not found: {}",
        archive.display()
    );
    let build_dir = workspace
        .join("target/xtask/android")
        .join(arch.triple)
        .join("release");
    let object = build_dir.join("jni_glue.o");
    let output = paths::sdk()
        .join("jniLibs")
        .join(arch.abi)
        .join("libreseam-sdk.so");
    fs::create_dir_all(&build_dir)?;
    fs::create_dir_all(output.parent().expect("abi dir"))?;

    let toolchain = Toolchain::new(ndk::android_clang(arch.clang_prefix, api)?);
    toolchain.compile(&glue_source(), &[glue_include()], &object)?;
    toolchain.link(&object, &archive, &["-lm", "-llog", "-ldl"], &output)?;
    println!("Linked {}", output.display());
    Ok(())
}

/// The desktop JNI shim the `reseam-sdk` jvm artifact packages: the sdk's static library plus the
/// JNI glue, built against the JDK in `JAVA_HOME`.
pub fn build_jni_host() -> Result<()> {
    let java_home = ndk::java_home()?;
    let workspace = paths::workspace_root();
    let glue = glue_source();
    ensure!(
        glue.is_file(),
        "JNI glue not found at {}; run `cargo xtask regen sdk` first",
        glue.display()
    );
    run(Command::new("cargo")
        .args(["build", "-p", "reseam-sdk", "--release"])
        .current_dir(&workspace)
        .env("JAVA_HOME", &java_home))?;

    let out_dir = workspace.join("target/release");
    let archive = out_dir.join("libreseam_sdk.a");
    let object = out_dir.join("reseam_sdk_jni_glue.o");
    let output = out_dir.join("libreseam_sdk_jni.so");
    let toolchain = Toolchain::new("cc");
    toolchain.compile(
        &glue,
        &[
            glue_include(),
            java_home.join("include"),
            java_home.join("include/linux"),
        ],
        &object,
    )?;
    toolchain.link(&object, &archive, &["-lpthread", "-ldl", "-lm"], &output)?;
    println!("Built: {}", output.display());
    Ok(())
}
