// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{bail, Context, Result};
use std::env;
use std::path::PathBuf;

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
        .and_then(|s| s.parse().ok())
        .unwrap_or(24)
}

pub fn find_android_clang(prefix: &str, api: u32) -> Result<PathBuf> {
    let name = format!("{prefix}{api}-clang");

    if let Ok(found) = which(&name) {
        return Ok(found);
    }

    let android_home = env::var_os("ANDROID_HOME")
        .map(PathBuf::from)
        .with_context(|| {
            format!("Android clang not found for {name}; set ANDROID_HOME or put the NDK llvm bin directory on PATH")
        })?;

    let ndk_root = android_home.join("ndk");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&ndk_root)
        .with_context(|| format!("reading {}", ndk_root.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    versions.sort_by(|a, b| version_key(a).cmp(&version_key(b)));

    for ndk in versions.iter().rev() {
        let prebuilt = ndk.join("toolchains/llvm/prebuilt");
        let Ok(entries) = std::fs::read_dir(&prebuilt) else {
            continue;
        };
        for host in entries.flatten() {
            let candidate = host.path().join("bin").join(&name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    bail!(
        "Android clang not found for {name}; install an NDK under {} or put the toolchain on PATH",
        ndk_root.display()
    );
}

fn version_key(path: &std::path::Path) -> Vec<u32> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            n.split('.')
                .map(|p| p.parse::<u32>().unwrap_or(0))
                .collect()
        })
        .unwrap_or_default()
}

fn which(name: &str) -> Result<PathBuf> {
    let path = env::var_os("PATH").context("PATH not set")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    bail!("{name} not on PATH");
}
