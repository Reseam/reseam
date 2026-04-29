// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ndk::{self, AndroidArch, ANDROID_ARCHES};
use crate::paths;
use crate::run::Cmd;

const EXPORTS_MAP: &str = r#"{
    global:
        Java_*;
        JNI_OnLoad*;
        JNI_OnUnload*;
        boltffi_*;
    local:
        *;
};
"#;

pub fn regen() -> Result<()> {
    let sdk = paths::sdk();

    Cmd::new("boltffi")
        .args(["build", "android", "--release"])
        .cwd(&sdk)
        .run()?;

    Cmd::new("boltffi")
        .args(["generate", "kotlin"])
        .cwd(&sdk)
        .run()?;

    fix_generated_jni()?;

    Cmd::new("boltffi")
        .args(["generate", "header"])
        .cwd(&sdk)
        .run()?;

    pack_android_jni_libs()?;
    println!("Regenerated SDK Kotlin bindings and Android jniLibs.");
    Ok(())
}

fn fix_generated_jni() -> Result<()> {
    let jni_path = paths::sdk().join("generated/jni/jni_glue.c");
    let kotlin_path = paths::sdk().join("generated/app/reseam/sdk/ReseamSdk.kt");

    if !jni_path.is_file() {
        bail!("generated JNI glue not found: {}", jni_path.display());
    }
    if !kotlin_path.is_file() {
        bail!("generated Kotlin binding not found: {}", kotlin_path.display());
    }

    let jni = fs::read_to_string(&jni_path)?;
    let kotlin = fs::read_to_string(&kotlin_path)?;

    if kotlin.contains("fun boltffiFutureContinuationCallback(") {
        println!(
            "Generated Kotlin has async continuation support; JNI glue does not need the sync-callback fix."
        );
        return Ok(());
    }

    if jni.contains("_poll(") || jni.contains("SubscriptionHandle") {
        bail!(
            "generated JNI glue contains async/stream polling but Kotlin has no continuation callback"
        );
    }

    const OLD: &str = r#"    if (boltffi_lookup_global_class(env, "app/reseam/sdk/Native", &g_callback_class) != BOLTFFI_GLOBAL_CLASS_OK) {
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

    const NEW: &str = r#"    /*
     * BoltFFI 0.24.1 emits this lookup whenever a callback trait exists, even
     * when the module has only synchronous callbacks. Kotlin correctly omits
     * boltffiFutureContinuationCallback unless async functions/streams/async
     * callbacks exist, so keep JNI_OnLoad limited to the sync callback setup.
     * Remove this once BoltFFI gates the JNI continuation lookup the same way
     * as the Kotlin Native template.
     */
"#;

    if jni.contains(NEW) {
        println!("Fixed: {}", jni_path.display());
        return Ok(());
    }
    if !jni.contains(OLD) {
        bail!(
            "BoltFFI JNI continuation lookup template changed; update xtask::sdk::fix_generated_jni"
        );
    }

    fs::write(&jni_path, jni.replace(OLD, NEW))?;
    println!("Fixed: {}", jni_path.display());
    Ok(())
}

fn pack_android_jni_libs() -> Result<()> {
    let api = ndk::android_api();
    for arch in ANDROID_ARCHES {
        link_android_jni_lib(arch, api)?;
    }
    Ok(())
}

fn link_android_jni_lib(arch: &AndroidArch, api: u32) -> Result<()> {
    let clang = ndk::find_android_clang(arch.clang_prefix, api)?;

    let workspace = paths::workspace_root();
    let build_dir = workspace
        .join("target/xtask/android")
        .join(arch.triple)
        .join("release");
    let object = build_dir.join("jni_glue.o");
    let exports = build_dir.join("exports.map");
    let source = paths::sdk().join("generated/jni/jni_glue.c");
    let include = paths::sdk().join("dist/android/include");
    let library = workspace
        .join("target")
        .join(arch.triple)
        .join("release/libreseam_sdk.a");
    let abi_dir = paths::sdk().join("jniLibs").join(arch.abi);
    let output = abi_dir.join("libreseam-sdk.so");

    if !library.is_file() {
        bail!(
            "built Android static library not found: {}",
            library.display()
        );
    }

    fs::create_dir_all(&build_dir)?;
    fs::create_dir_all(&abi_dir)?;
    fs::write(&exports, EXPORTS_MAP)?;

    Cmd::new(&clang)
        .args(["-c", "-fPIC", "-O3"])
        .arg("-I").arg(&include)
        .arg(&source)
        .arg("-o").arg(&object)
        .run()?;

    Cmd::new(&clang)
        .arg("-shared")
        .arg("-o").arg(&output)
        .arg(&object)
        .arg("-Wl,--whole-archive").arg(&library).arg("-Wl,--no-whole-archive")
        .arg("-Xlinker").arg("--version-script")
        .arg("-Xlinker").arg(&exports)
        .arg("-Wl,--gc-sections")
        .args(["-lm", "-llog", "-ldl"])
        .run()?;

    println!("Linked {}", display_relative(&output, &workspace));
    Ok(())
}

fn display_relative(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

pub fn build_jni_host() -> Result<()> {
    let java_home = env::var_os("JAVA_HOME")
        .map(PathBuf::from)
        .context("JAVA_HOME not set")?;

    let workspace = paths::workspace_root();
    let jni_src = paths::sdk().join("generated/jni/jni_glue.c");
    let header_dir = paths::sdk().join("dist/android/include");
    let out_dir = workspace.join("target/release");
    let static_lib = out_dir.join("libreseam_sdk.a");

    if !jni_src.is_file() {
        bail!(
            "JNI glue not found at {}; run `cargo xtask regen sdk` first",
            jni_src.display()
        );
    }

    Cmd::new("cargo")
        .args(["build", "-p", "reseam-sdk", "--release"])
        .cwd(&workspace)
        .env("JAVA_HOME", &java_home)
        .run()?;

    if !static_lib.is_file() {
        bail!(
            "static library not produced at {}; reseam-sdk Cargo.toml must declare crate-type \"staticlib\"",
            static_lib.display()
        );
    }

    let (jni_include, ext) = if cfg!(target_os = "macos") {
        (java_home.join("include/darwin"), "dylib")
    } else if cfg!(target_os = "linux") {
        (java_home.join("include/linux"), "so")
    } else {
        bail!("unsupported host OS for JNI host build");
    };

    let object = out_dir.join("reseam_sdk_jni_glue.o");
    let output = out_dir.join(format!("libreseam_sdk_jni.{ext}"));
    let exports = out_dir.join("reseam_sdk_jni.exports");
    fs::write(&exports, EXPORTS_MAP)?;

    println!("Compiling JNI glue...");
    Cmd::new("cc")
        .args(["-c", "-fPIC", "-O3", "-w"])
        .arg("-I").arg(&header_dir)
        .arg("-I").arg(java_home.join("include"))
        .arg("-I").arg(&jni_include)
        .arg("-o").arg(&object)
        .arg(&jni_src)
        .run()?;

    println!("Linking {}...", display_relative(&output, &workspace));
    let mut link = Cmd::new("cc")
        .arg("-shared")
        .arg("-o").arg(&output)
        .arg(&object);

    link = if cfg!(target_os = "macos") {
        link
            .arg("-Wl,-force_load").arg(&static_lib)
            .args(["-framework", "Security", "-framework", "CoreFoundation"])
            .args(["-lpthread", "-ldl", "-lm"])
    } else {
        link
            .arg("-Wl,--whole-archive").arg(&static_lib).arg("-Wl,--no-whole-archive")
            .arg("-Xlinker").arg("--version-script")
            .arg("-Xlinker").arg(&exports)
            .arg("-Wl,--gc-sections")
            .args(["-lpthread", "-ldl", "-lm"])
    };
    link.run()?;

    fs::remove_file(&object).ok();
    println!("Built: {}", display_relative(&output, &workspace));
    Ok(())
}
