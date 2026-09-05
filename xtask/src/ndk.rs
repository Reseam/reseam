// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

pub struct AndroidArch {
    pub triple: &'static str,
    pub abi: &'static str,
    pub clang_prefix: &'static str,
}

pub const ANDROID_ARCHES: &[AndroidArch] = &[
    AndroidArch {
        triple: "aarch64-linux-android",
        abi: "arm64-v8a",
        clang_prefix: "aarch64-linux-android",
    },
    AndroidArch {
        triple: "armv7-linux-androideabi",
        abi: "armeabi-v7a",
        clang_prefix: "armv7a-linux-androideabi",
    },
    AndroidArch {
        triple: "i686-linux-android",
        abi: "x86",
        clang_prefix: "i686-linux-android",
    },
    AndroidArch {
        triple: "x86_64-linux-android",
        abi: "x86_64",
        clang_prefix: "x86_64-linux-android",
    },
];

pub fn android_api() -> u32 {
    env::var("ANDROID_API")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(24)
}

/// The NDK's `<prefix><api>-clang`, from PATH or the newest NDK under
/// `$ANDROID_HOME/ndk`.
pub fn android_clang(prefix: &str, api: u32) -> Result<PathBuf> {
    let name = format!("{prefix}{api}-clang");
    if let Some(found) = env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|dir| dir.join(&name))
            .find(|candidate| candidate.is_file())
    }) {
        return Ok(found);
    }

    let ndk_root = env::var_os("ANDROID_HOME")
        .map(|home| PathBuf::from(home).join("ndk"))
        .with_context(|| {
            format!("{name} not on PATH and ANDROID_HOME is not set; install an NDK or put its llvm bin directory on PATH")
        })?;
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&ndk_root)
        .with_context(|| format!("reading {}", ndk_root.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    versions.sort_by_key(|path| version_key(path));

    versions
        .iter()
        .rev()
        .filter_map(|ndk| std::fs::read_dir(ndk.join("toolchains/llvm/prebuilt")).ok())
        .flatten()
        .flatten()
        .map(|host| host.path().join("bin").join(&name))
        .find(|candidate| candidate.is_file())
        .with_context(|| format!("{name} not found in any NDK under {}", ndk_root.display()))
}

fn version_key(path: &Path) -> Vec<u32> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            name.split('.')
                .map(|part| part.parse().unwrap_or(0))
                .collect()
        })
        .unwrap_or_default()
}

pub fn java_home() -> Result<PathBuf> {
    env::var_os("JAVA_HOME")
        .map(PathBuf::from)
        .context("JAVA_HOME not set")
        .and_then(|home| {
            if cfg!(target_os = "linux") {
                Ok(home)
            } else {
                bail!("the JNI host build supports Linux only")
            }
        })
}
