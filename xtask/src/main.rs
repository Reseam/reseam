// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

mod boltffi;
mod browser;
mod dex_codec;
mod framework;
mod models;
mod patch_api;
mod paths;
mod run;
mod sdk;
mod version;

#[derive(Parser)]
#[command(name = "xtask", about = "Reseam build orchestration tasks")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Regenerates Kotlin bindings and JNI sources using the host toolchain.
    Regen {
        #[arg(value_enum)]
        target: RegenTarget,
    },
    /// Builds the patch API and Kotlin runtime jar embedded in desktop engine builds, and the app
    /// runtime every engine build embeds.
    Runtime,
    /// Packages SDK Android and desktop JNI libraries after building the runtime jar.
    PackSdk,
    /// Builds the browser engine and runtime assets into `browser/public/runtime`. Needs `WASI_SDK_PATH`.
    PackBrowser,
    /// Sets the workspace version, commits, and tags the release.
    Release { version: String },
    /// Fails unless the tag names the workspace version.
    CheckTag { tag: String },
    /// Checks that decoding then encoding every method of each DEX, or of each
    /// `classes*.dex` in an APK, reproduces its code units.
    DexRoundTrip { paths: Vec<PathBuf> },
    /// Generates framework resource metadata from Android SDK platform 36, revision 2.
    FrameworkAttributes {
        #[arg(long)]
        android_jar: PathBuf,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum RegenTarget {
    PatchApi,
    Sdk,
    /// The browser package's TypeScript models.
    Browser,
    All,
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Regen { target } => {
            if !matches!(target, RegenTarget::Browser) {
                boltffi::check_version()?;
            }
            if matches!(target, RegenTarget::PatchApi | RegenTarget::All) {
                patch_api::regen()?;
            }
            if matches!(target, RegenTarget::Sdk | RegenTarget::All) {
                sdk::regen()?;
            }
            if matches!(target, RegenTarget::Browser | RegenTarget::All) {
                models::regen()?;
            }
        }
        Cmd::Runtime => runtime()?,
        Cmd::PackSdk => {
            boltffi::check_version()?;
            sdk::pack()?;
        }
        Cmd::PackBrowser => {
            boltffi::check_version()?;
            browser::pack()?;
        }
        Cmd::Release { version } => version::tag(&version)?,
        Cmd::CheckTag { tag } => version::check_tag(&tag)?,
        Cmd::DexRoundTrip { paths } => dex_codec::round_trip(&paths)?,
        Cmd::FrameworkAttributes { android_jar } => framework::generate(&android_jar)?,
    }
    Ok(())
}

fn runtime() -> Result<()> {
    run::gradle(&[":reseam-sdk:patchRuntimeJar", ":app-runtime:stage"])
}
