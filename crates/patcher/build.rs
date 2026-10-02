// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::error::Error;
use std::path::PathBuf;

#[path = "../../build-support/java.rs"]
mod java;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../build-support/java.rs");
    println!("cargo:rerun-if-env-changed=BOLTFFI_BINDING_METADATA");
    println!("cargo:rustc-check-cfg=cfg(reseam_jni_bridge)");
    if env::var_os("CARGO_FEATURE_BRIDGE").is_none() {
        return Ok(());
    }
    let out = PathBuf::from(env::var("OUT_DIR")?);
    let runtime = out.join("runtime.jar");
    if env::var_os("CARGO_FEATURE_KOTLIN").is_some()
        && env::var("CARGO_CFG_TARGET_OS")? != "android"
    {
        if env::var_os("BOLTFFI_BINDING_METADATA").is_some() {
            std::fs::write(runtime, [])?;
        } else {
            println!("cargo:rerun-if-env-changed=RESEAM_RUNTIME_JAR");
            let source = env::var_os("RESEAM_RUNTIME_JAR").map_or_else(
                || {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../build/runtime/reseam-runtime.jar")
                },
                PathBuf::from,
            );
            println!("cargo:rerun-if-changed={}", source.display());
            std::fs::copy(&source, runtime).map_err(|error| format!(
                "patch runtime jar {} is unavailable: {error}; run `cargo xtask runtime` before building desktop hosts", source.display()
            ))?;
        }
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

    if env::var("CARGO_CFG_TARGET_OS")? == "wasi" {
        let generated = manifest_dir.join("../../patch-api/generated");
        let transport = manifest_dir.join("browser");
        println!("cargo:rerun-if-changed={}", generated.display());
        println!("cargo:rerun-if-changed={}", transport.display());
        cc::Build::new()
            .file(generated.join("browser/bridge.c"))
            .include(&transport)
            .include(generated.join("jni"))
            .flag_if_supported("-Wno-unused-parameter")
            .compile("reseam_browser_bridge");
        return Ok(());
    }

    let jni_dir = manifest_dir.join("../../patch-api/generated/jni");
    let registration = jni_dir.join("registration.c");
    println!("cargo:rerun-if-changed={}", jni_dir.display());
    if !registration.exists() {
        return Err("JNI bridge is missing; run `cargo xtask regen patch-api` first".into());
    }

    let mut build = cc::Build::new();
    build.file(&registration).include(&jni_dir);
    // The NDK's clang wrappers carry their own sysroot with jni.h.
    if env::var("CARGO_CFG_TARGET_OS")? != "android" {
        let target = env::var("TARGET")?;
        for variable in java::variables(&target) {
            println!("cargo:rerun-if-env-changed={variable}");
        }
        let java_home = java::home(&target)?;
        let include = java_home.join("include");
        let platform = include.join(java::platform(&target));
        println!("cargo:rerun-if-changed={}", include.join("jni.h").display());
        println!(
            "cargo:rerun-if-changed={}",
            platform.join("jni_md.h").display()
        );
        build.include(include).include(platform);
    }
    // Generated JNI entry points keep the standard `env`/`cls` parameters even
    // when a call uses neither.
    build.flag_if_supported("-Wno-unused-parameter");
    build.compile("reseam_jni_bridge");
    println!("cargo:rustc-cfg=reseam_jni_bridge");
    Ok(())
}
