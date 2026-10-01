// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use boltffi_binding::{
    BINDING_EXPANSION_BUILD_ENV, BINDING_EXPANSION_ROOT_ENV, BINDING_EXPANSION_SOURCE_ENV,
    BINDING_EXPANSION_SURFACE_ENV,
};

use crate::{
    paths,
    run::{output, run},
};

#[path = "../../build-support/java.rs"]
mod java;

struct DesktopTarget {
    triple: &'static str,
    platform: &'static str,
    library: &'static str,
    link_args: &'static [&'static str],
}

const DESKTOP_TARGETS: [DesktopTarget; 2] = [
    DesktopTarget {
        triple: "x86_64-unknown-linux-gnu",
        platform: "linux-x86_64",
        library: "libreseam_sdk_native_jni.so",
        link_args: &[],
    },
    DesktopTarget {
        triple: "x86_64-pc-windows-gnullvm",
        platform: "windows-x86_64",
        library: "reseam_sdk_native_jni.dll",
        link_args: &["-static"],
    },
];

/// Generates Kotlin and JNI source for the application SDK on the host toolchain.
pub fn regen() -> Result<()> {
    run(Command::new("boltffi")
        .args(["generate", "kotlin", "--deny-skipped"])
        .current_dir(paths::workspace_root().join("sdk/native")))?;
    let generated = paths::workspace_root().join("sdk/generated/app/reseam/sdk/ReseamSdk.kt");
    let hook = format!("-PspotlessIdeHook={}", generated.display());
    crate::run::gradle(&["spotlessKotlinApply", &hook])
}

/// Packages Android and both release desktop targets with the pinned generator.
/// Requires the runtime jar, NDK, Android and Windows Rust targets, llvm-mingw,
/// the host JDK in `JAVA_HOME` and a Windows JDK in its per-target override.
pub fn pack() -> Result<()> {
    let root = paths::workspace_root();
    let targets = DESKTOP_TARGETS
        .iter()
        .map(|target| {
            java::home(target.triple)
                .map(|jdk| (target, jdk))
                .map_err(|error| anyhow::anyhow!("{error}"))
        })
        .collect::<Result<Vec<_>>>()?;
    run(Command::new("boltffi")
        .args([
            "pack",
            "android",
            "--release",
            "--deny-skipped",
            "--cargo-arg=--config",
            "--cargo-arg=build.target=\"x86_64-pc-windows-gnullvm\"",
        ])
        .current_dir(root.join("sdk/native")))?;

    let host = output(Command::new("rustc").arg("--print=host-tuple"))?;
    let host = std::str::from_utf8(&host.stdout)?.trim();
    for (target, jdk) in targets {
        pack_desktop(target, host, &jdk)?;
    }
    Ok(())
}

fn pack_desktop(target: &DesktopTarget, host: &str, jdk: &Path) -> Result<()> {
    writeln!(
        std::io::stdout(),
        "Building desktop JNI for {}",
        target.platform
    )?;
    let root = paths::workspace_root();
    let native = root.join("sdk/native");
    let built = output(
        Command::new("cargo")
            .args([
                "rustc",
                "--lib",
                "--release",
                "--color=never",
                "-p",
                "reseam-sdk-native",
                "--target",
                target.triple,
                "--",
                "--cfg",
                "boltffi_binding_expansion",
                "--print=native-static-libs",
            ])
            .env(BINDING_EXPANSION_BUILD_ENV, "1")
            .env(BINDING_EXPANSION_ROOT_ENV, &native)
            .env(BINDING_EXPANSION_SOURCE_ENV, native.join("src/lib.rs"))
            .env(BINDING_EXPANSION_SURFACE_ENV, "native")
            .current_dir(&root),
    )?;
    std::io::stderr().write_all(&built.stderr)?;
    let stderr = std::str::from_utf8(&built.stderr)?;
    let libraries = stderr
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix("note: native-static-libs: "))
        .context("cargo rustc did not report native static library dependencies")?;
    let target_dir =
        env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), |path| root.join(path));
    let archive = target_dir
        .join(target.triple)
        .join("release/libreseam_sdk_native.a");
    let output_dir = root
        .join("sdk/dist/android/desktopJniLibs")
        .join(target.platform);
    fs::create_dir_all(&output_dir)?;
    let jni = root.join("sdk/generated/jni");
    let mut command = cc::Build::new()
        .host(host)
        .target(target.triple)
        .opt_level(3)
        .debug(false)
        .warnings(false)
        .pic(true)
        .cargo_metadata(false)
        .try_get_compiler()?
        .to_command();
    run(command
        .args(["-shared", "-Wl,--no-undefined", "-std=c11", "-o"])
        .arg(output_dir.join(target.library))
        .arg(jni.join("jni_glue.c"))
        .arg(archive)
        .arg("-I")
        .arg(&jni)
        .arg("-I")
        .arg(jdk.join("include"))
        .arg("-I")
        .arg(jdk.join("include").join(java::platform(target.triple)))
        .args(target.link_args)
        .args(libraries.split_whitespace()))
}
