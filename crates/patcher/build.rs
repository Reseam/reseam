// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[path = "../../build-support/java.rs"]
mod java;

/// The Kotlin version the patch runtime is compiled with, from the Gradle version catalog.
fn export_kotlin_version(manifest_dir: &Path) -> Result<(), Box<dyn Error>> {
    let catalog = manifest_dir.join("../../gradle/libs.versions.toml");
    println!("cargo:rerun-if-changed={}", catalog.display());
    let catalog: toml::Table = std::fs::read_to_string(&catalog)?.parse()?;
    let version = catalog
        .get("versions")
        .and_then(|versions| versions.get("kotlin"))
        .and_then(toml::Value::as_str)
        .ok_or("gradle/libs.versions.toml names no [versions] kotlin")?;
    println!("cargo:rustc-env=RESEAM_KOTLIN_VERSION={version}");
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../build-support/java.rs");
    println!("cargo:rerun-if-env-changed=BOLTFFI_BINDING_METADATA");
    println!("cargo:rustc-check-cfg=cfg(reseam_jni_bridge)");
    if env::var_os("CARGO_FEATURE_BRIDGE").is_none() {
        return Ok(());
    }
    let out = PathBuf::from(env::var("OUT_DIR")?);
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    embed_app_runtime(&out, &manifest_dir)?;
    let runtime = out.join("runtime.jar");
    if env::var_os("CARGO_FEATURE_KOTLIN").is_some()
        && env::var("CARGO_CFG_TARGET_OS")? != "android"
    {
        if env::var_os("BOLTFFI_BINDING_METADATA").is_some() {
            std::fs::write(runtime, [])?;
        } else {
            println!("cargo:rerun-if-env-changed=RESEAM_RUNTIME_JAR");
            let source = env::var_os("RESEAM_RUNTIME_JAR").map_or_else(
                || manifest_dir.join("../../build/runtime/reseam-runtime.jar"),
                PathBuf::from,
            );
            println!("cargo:rerun-if-changed={}", source.display());
            std::fs::copy(&source, runtime).map_err(|error| format!(
                "patch runtime jar {} is unavailable: {error}; run `cargo xtask runtime` before building desktop hosts", source.display()
            ))?;
        }
        export_kotlin_version(&manifest_dir)?;
    }
    // BoltFFI reads Binding IR from a metadata build, which has no bridge yet.
    if env::var_os("BOLTFFI_BINDING_METADATA").is_some() {
        return Ok(());
    }

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

/// Writes `app_runtime.rs`, which embeds the app runtime that `cargo xtask runtime` stages: its DEX
/// and its Hermes bytecode for each supported version.
fn embed_app_runtime(out: &Path, manifest_dir: &Path) -> Result<(), Box<dyn Error>> {
    let generated = out.join("app_runtime.rs");
    if env::var_os("BOLTFFI_BINDING_METADATA").is_some() {
        std::fs::write(
            generated,
            "pub(crate) const DEX: &[u8] = &[];\npub(crate) const HERMES: &[(u32, &[u8])] = &[];\n",
        )?;
        return Ok(());
    }
    println!("cargo:rerun-if-env-changed=RESEAM_APP_RUNTIME");
    let source = env::var_os("RESEAM_APP_RUNTIME").map_or_else(
        || manifest_dir.join("../../build/runtime/app"),
        PathBuf::from,
    );
    println!("cargo:rerun-if-changed={}", source.display());
    let missing = |path: &Path| {
        format!(
            "app runtime {} is unavailable; run `cargo xtask runtime` before building the engine",
            path.display()
        )
    };
    let dex = source.join("reseam-app-runtime.dex");
    if !dex.is_file() {
        return Err(missing(&dex).into());
    }
    let hermes_dir = source.join("hermes");
    let mut hermes = Vec::new();
    for entry in std::fs::read_dir(&hermes_dir).map_err(|_| missing(&hermes_dir))? {
        let entry = entry?;
        let name = entry.file_name();
        let version = name
            .to_str()
            .and_then(|name| name.strip_prefix('v'))
            .and_then(|version| version.parse::<u32>().ok())
            .ok_or_else(|| format!("unexpected Hermes directory {}", entry.path().display()))?;
        let module = entry.path().join("reseam-app-runtime-js.hbc");
        if !module.is_file() {
            return Err(missing(&module).into());
        }
        hermes.push((version, module));
    }
    hermes.sort();
    let mut code = format!(
        "pub(crate) const DEX: &[u8] = include_bytes!({});\n",
        literal(&dex)?
    );
    code.push_str("pub(crate) const HERMES: &[(u32, &[u8])] = &[\n");
    for (version, module) in hermes {
        writeln!(
            code,
            "    ({version}, include_bytes!({})),",
            literal(&module)?
        )?;
    }
    code.push_str("];\n");
    std::fs::write(generated, code)?;
    Ok(())
}

/// An absolute path as a Rust string literal.
fn literal(path: &Path) -> Result<String, Box<dyn Error>> {
    let path = path.canonicalize()?;
    let path = path
        .to_str()
        .ok_or_else(|| format!("{} is not UTF-8", path.display()))?;
    Ok(format!("{path:?}"))
}
