// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::paths;
use crate::run::run;

pub fn regen() -> Result<()> {
    let patcher = paths::patcher_crate();
    run(Command::new("boltffi")
        .args(["generate", "kotlin"])
        .env("RESEAM_SKIP_JNI_GLUE", "1")
        .current_dir(&patcher))?;
    run(Command::new("boltffi")
        .args(["generate", "header", "-o"])
        .arg(paths::patch_api().join("generated/jni"))
        .env("RESEAM_SKIP_JNI_GLUE", "1")
        .current_dir(&patcher))?;
    publish_bridge()?;
    println!("Regenerated patch-api Kotlin bridge and JNI headers.");
    Ok(())
}

const SPDX_HEADER: &str = "// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>\n// SPDX-License-Identifier: GPL-3.0-or-later\n\n";

const GENERATED_LOADER: &str = r#"    init {
        val preferredLibrary = "reseam_patcher_jni"
        val fallbackLibrary = "reseam_patcher"
        val vmName = System.getProperty("java.vm.name").orEmpty()
        val isAndroidRuntime =
            vmName.contains("dalvik", ignoreCase = true) ||
            vmName.contains("art", ignoreCase = true)
        if (isAndroidRuntime) {
            System.loadLibrary(fallbackLibrary)
        } else {
            loadDesktopLibraries(preferredLibrary, fallbackLibrary)
        }
    }
"#;

/// Patch code only ever runs inside the engine, which registers the bridge's
/// natives on `Native` itself before loading any patch class. The bridge must
/// not load a library of its own on any platform.
const HOST_REGISTERED_LOADER: &str = "";

/// Copies the generated bridge into the patch-api sources with the license
/// header, unused imports dropped, and the native loader removed.
fn publish_bridge() -> Result<()> {
    let src = paths::patch_api().join("generated/app/reseam/patch/ReseamPatcher.kt");
    let dst = paths::patch_api().join("src/main/kotlin/app/reseam/patch/ReseamPatcher.kt");
    let mut content =
        fs::read_to_string(&src).with_context(|| format!("reading {}", src.display()))?;

    if !content.starts_with("// SPDX-FileCopyrightText:") {
        content.insert_str(0, SPDX_HEADER);
    }
    for (import, symbol) in [
        (
            "java.util.concurrent.ConcurrentHashMap",
            "ConcurrentHashMap",
        ),
        ("java.util.concurrent.atomic.AtomicBoolean", "AtomicBoolean"),
        ("java.util.concurrent.atomic.AtomicLong", "AtomicLong"),
    ] {
        let stripped = content.replace(&format!("import {import}\n"), "");
        if !stripped.contains(symbol) {
            content = stripped;
        }
    }
    if !content.contains(GENERATED_LOADER) {
        bail!("BoltFFI's native loader template changed; update xtask::patch_api");
    }
    content = content.replace(GENERATED_LOADER, HOST_REGISTERED_LOADER);

    fs::write(&dst, content).with_context(|| format!("writing {}", dst.display()))?;
    println!("Synced bridge to {}", dst.display());
    Ok(())
}
