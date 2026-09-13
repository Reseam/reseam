// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(reseam_skip_jni_glue)");
    println!("cargo:rerun-if-env-changed=RESEAM_SKIP_JNI_GLUE");

    if env::var("CARGO_FEATURE_KOTLIN").is_err() || !should_compile_jni_glue() {
        println!("cargo:rustc-cfg=reseam_skip_jni_glue");
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    // This is a second binding root embedded in the SDK/CLI. BoltFFI normally
    // suppresses exports in dependency crates; retain this root's independent
    // runtime contract so the hosted registration table always has definitions.
    if env::var_os("BOLTFFI_BINDING_METADATA").is_none() {
        println!("cargo:rustc-env=BOLTFFI_BINDING_EXPANSION=1");
        println!(
            "cargo:rustc-env=BOLTFFI_BINDING_EXPANSION_ROOT={}",
            manifest_dir.display()
        );
        println!(
            "cargo:rustc-env=BOLTFFI_BINDING_EXPANSION_SOURCE={}",
            manifest_dir.join("src/lib.rs").display()
        );
        println!("cargo:rustc-env=BOLTFFI_BINDING_EXPANSION_SURFACE=native");
    }
    println!("cargo:rerun-if-env-changed=BOLTFFI_BINDING_METADATA");
    let jni_dir = manifest_dir.join("../../patch-api/generated/jni");
    let glue_path = jni_dir.join("registration.c");
    println!("cargo:rerun-if-changed={}", jni_dir.display());
    println!("cargo:rerun-if-changed={}", glue_path.display());

    if !glue_path.exists() {
        return Err(
            "JNI bridge is missing; run `cargo xtask regen patch-api` before building the engine"
                .into(),
        );
    }

    let jni_includes = jni_include_dirs()?;

    let mut build = cc::Build::new();
    build.file(&glue_path).include(&jni_dir);
    for include in jni_includes {
        build.include(include);
    }

    // The JNI glue is generated and intentionally keeps the standard JNI
    // parameter shape even when some exports do not use `env`/`cls`.
    build.flag_if_supported("-Wno-unused-parameter");

    build.compile("reseam_jni_glue");

    Ok(())
}

fn should_compile_jni_glue() -> bool {
    env::var_os("RESEAM_SKIP_JNI_GLUE").is_none()
}

fn jni_include_dirs() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("android") {
        return android_jni_include_dirs();
    }

    let java_home = find_java_home()?;
    let jni_include = PathBuf::from(&java_home).join("include");
    let jni_platform = if target.contains("apple-darwin") {
        jni_include.join("darwin")
    } else if target.contains("windows") {
        jni_include.join("win32")
    } else {
        jni_include.join("linux")
    };
    Ok(vec![jni_include, jni_platform])
}

fn android_jni_include_dirs() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let ndk = resolve_android_ndk()
        .ok_or("Android NDK not found; set ANDROID_NDK_HOME or ANDROID_HOME/ANDROID_SDK_ROOT")?;
    let prebuilt = ndk.join("toolchains").join("llvm").join("prebuilt");
    let host = std::fs::read_dir(&prebuilt)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| path.join("sysroot").join("usr").join("include").is_dir())
        .ok_or_else(|| format!("Android NDK sysroot not found under {}", prebuilt.display()))?;
    Ok(vec![host.join("sysroot").join("usr").join("include")])
}

fn resolve_android_ndk() -> Option<PathBuf> {
    env::var_os("ANDROID_NDK_HOME")
        .map(PathBuf::from)
        .filter(|path| is_android_ndk(path))
        .or_else(|| {
            let sdk = env::var_os("ANDROID_HOME")
                .or_else(|| env::var_os("ANDROID_SDK_ROOT"))
                .map(PathBuf::from)?;
            let ndk_bundle = sdk.join("ndk-bundle");
            if is_android_ndk(&ndk_bundle) {
                return Some(ndk_bundle);
            }
            let ndk_dir = sdk.join("ndk");
            let mut versions = std::fs::read_dir(ndk_dir)
                .ok()?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| is_android_ndk(path))
                .collect::<Vec<_>>();
            versions.sort();
            versions.pop()
        })
}

fn is_android_ndk(path: &Path) -> bool {
    path.join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .is_dir()
}

fn find_java_home() -> Result<String, Box<dyn Error>> {
    if let Ok(home) = env::var("JAVA_HOME") {
        if Path::new(&home).is_dir() {
            return Ok(home);
        }
    }
    if let Ok(output) = std::process::Command::new("java")
        .arg("-XshowSettings:property")
        .arg("-version")
        .output()
    {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("java.home") {
                if let Some(val) = trimmed.split('=').nth(1) {
                    let path = val.trim();
                    if Path::new(path).is_dir() {
                        return Ok(path.to_string());
                    }
                }
            }
        }
    }
    Err("JAVA_HOME not set and java not found on PATH".into())
}
