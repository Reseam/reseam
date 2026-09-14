// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Compiles the BoltFFI JNI bridge the hosted patch runtime registers on each
//! bundle's `Native` class. `cargo xtask regen patch-api` generates it.

use std::env;
use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BOLTFFI_BINDING_METADATA");
    if env::var_os("CARGO_FEATURE_KOTLIN").is_none() {
        return Ok(());
    }
    // BoltFFI reads Binding IR from a metadata build, which has no bridge yet.
    if env::var_os("BOLTFFI_BINDING_METADATA").is_some() {
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    // BoltFFI emits exported functions only for the crate it generates bindings
    // for. The patcher is a second binding root inside the SDK and the CLI, so
    // it declares itself the root of its own expansion.
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

    let jni_dir = manifest_dir.join("../../patch-api/generated/jni");
    let registration = jni_dir.join("registration.c");
    println!("cargo:rerun-if-changed={}", jni_dir.display());
    if !registration.exists() {
        return Err("JNI bridge is missing; run `cargo xtask regen patch-api` first".into());
    }

    let mut build = cc::Build::new();
    build.file(&registration).include(&jni_dir);
    // The NDK's clang wrappers carry their own sysroot with jni.h.
    if !env::var("TARGET")?.contains("android") {
        let java_home = PathBuf::from(env::var("JAVA_HOME").map_err(|_| "JAVA_HOME is not set")?);
        let platform = match env::var("CARGO_CFG_TARGET_OS")?.as_str() {
            "macos" => "darwin",
            "windows" => "win32",
            _ => "linux",
        };
        build
            .include(java_home.join("include"))
            .include(java_home.join("include").join(platform));
    }
    // Generated JNI entry points keep the standard `env`/`cls` parameters even
    // when a call uses neither.
    build.flag_if_supported("-Wno-unused-parameter");
    build.compile("reseam_jni_bridge");
    Ok(())
}
