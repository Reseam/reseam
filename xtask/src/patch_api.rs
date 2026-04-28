// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::paths;
use crate::run::Cmd;

pub fn regen() -> Result<()> {
    let patcher = paths::patcher_crate();
    let generated_jni = paths::patch_api().join("generated/jni");

    Cmd::new("boltffi")
        .arg("generate")
        .arg("kotlin")
        .env("RESEAM_SKIP_JNI_GLUE", "1")
        .cwd(&patcher)
        .run()?;

    Cmd::new("boltffi")
        .arg("generate")
        .arg("header")
        .arg("-o")
        .arg(&generated_jni)
        .env("RESEAM_SKIP_JNI_GLUE", "1")
        .cwd(&patcher)
        .run()?;

    sync_published_bridge()?;
    println!("Regenerated patch-api Kotlin bridge and JNI headers.");
    Ok(())
}

fn sync_published_bridge() -> Result<()> {
    let src = paths::patch_api().join("generated/app/reseam/patch/ReseamPatcher.kt");
    let dst = paths::patch_api().join("src/main/kotlin/app/reseam/patch/ReseamPatcher.kt");

    if !src.is_file() {
        bail!("generated bridge not found at {}", src.display());
    }

    let mut content = fs::read_to_string(&src)
        .with_context(|| format!("reading {}", src.display()))?;

    const SPDX_HEADER: &str = "// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>\n// SPDX-License-Identifier: GPL-3.0-or-later\n\n";
    if !content.starts_with("// SPDX-FileCopyrightText:") {
        content.insert_str(0, SPDX_HEADER);
    }

    for (import, symbol) in [
        ("java.util.concurrent.ConcurrentHashMap", "ConcurrentHashMap"),
        ("java.util.concurrent.atomic.AtomicBoolean", "AtomicBoolean"),
        ("java.util.concurrent.atomic.AtomicLong", "AtomicLong"),
    ] {
        let line = format!("import {import}\n");
        let stripped = content.replace(&line, "");
        if !stripped.contains(symbol) {
            content = stripped;
        }
    }

    const OLD_LOADER: &str = r#"        val vmName = System.getProperty("java.vm.name").orEmpty()
        val isAndroidRuntime =
            vmName.contains("dalvik", ignoreCase = true) ||
            vmName.contains("art", ignoreCase = true)
        if (isAndroidRuntime) {
            System.loadLibrary(fallbackLibrary)
        } else {
            loadDesktopLibraries(preferredLibrary, fallbackLibrary)
        }
"#;
    const NEW_LOADER: &str = r#"        val vmName = System.getProperty("java.vm.name").orEmpty()
        val bootstrapMode = System.getProperty("reseam.native.bootstrap").orEmpty()
        val isAndroidRuntime =
            vmName.contains("dalvik", ignoreCase = true) ||
            vmName.contains("art", ignoreCase = true)
        if (isAndroidRuntime) {
            System.loadLibrary(fallbackLibrary)
        } else if (bootstrapMode != "host-registered") {
            loadDesktopLibraries(preferredLibrary, fallbackLibrary)
        }
"#;

    if content.contains(OLD_LOADER) {
        content = content.replace(OLD_LOADER, NEW_LOADER);
    } else if !content.contains(NEW_LOADER) {
        bail!(
            "BoltFFI native loader template changed; update host-registered bootstrap integration in xtask::patch_api"
        );
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&dst, content).with_context(|| format!("writing {}", dst.display()))?;
    println!("Synced bridge to {}", dst.display());
    Ok(())
}

pub fn build_jni_host() -> Result<()> {
    let java_home = env::var_os("JAVA_HOME")
        .map(PathBuf::from)
        .context("JAVA_HOME not set")?;

    let jni_src = paths::patch_api().join("generated/jni/jni_glue.c");
    let header_dir = paths::patch_api().join("generated/jni");
    let out_dir = paths::target_debug();

    if !jni_src.is_file() {
        bail!(
            "JNI glue not found at {}; run `cargo xtask regen patch-api` first",
            jni_src.display()
        );
    }

    let cdylib_name = if cfg!(target_os = "macos") {
        "libreseam_patcher.dylib"
    } else {
        "libreseam_patcher.so"
    };
    let cdylib = out_dir.join(cdylib_name);
    if !cdylib.is_file() {
        bail!(
            "cdylib not found at {}; run `JAVA_HOME=$JAVA_HOME cargo build -p reseam-patcher` first",
            cdylib.display()
        );
    }

    let (jni_include, ext, rpath) = if cfg!(target_os = "macos") {
        (java_home.join("include/darwin"), "dylib", "-Wl,-rpath,@loader_path")
    } else if cfg!(target_os = "linux") {
        (java_home.join("include/linux"), "so", "-Wl,-rpath,$ORIGIN")
    } else {
        bail!("unsupported host OS for JNI host build");
    };

    let object = out_dir.join("jni_glue.o");
    let output = out_dir.join(format!("libreseam_patcher_jni.{ext}"));

    fs::create_dir_all(&out_dir)?;

    println!("Compiling JNI glue...");
    Cmd::new("cc")
        .args(["-c", "-fPIC", "-w"])
        .arg("-I").arg(&header_dir)
        .arg("-I").arg(java_home.join("include"))
        .arg("-I").arg(&jni_include)
        .arg("-o").arg(&object)
        .arg(&jni_src)
        .run()?;

    println!("Linking {}...", output.display());
    Cmd::new("cc")
        .arg("-shared")
        .arg("-o").arg(&output)
        .arg(&object)
        .arg("-L").arg(&out_dir)
        .arg("-lreseam_patcher")
        .arg(rpath)
        .run()?;

    fs::remove_file(&object).ok();
    println!("Built: {}", output.display());
    Ok(())
}
