// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

mod boltffi;
mod framework;
mod opcodes;
mod patch_api;
mod paths;
mod release;
mod run;
mod sdk;

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
    /// Builds the patch API and Kotlin runtime jar embedded in desktop engine builds.
    Runtime,
    /// Packages SDK Android and desktop JNI libraries after building the runtime jar.
    PackSdk,
    /// Sets the workspace version, commits, and tags the release.
    Release { version: String },
    /// Fails unless the tag names the workspace version.
    CheckTag { tag: String },
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
    All,
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Regen { target } => {
            boltffi::check_version()?;
            if matches!(target, RegenTarget::PatchApi | RegenTarget::All) {
                patch_api::regen()?;
            }
            if matches!(target, RegenTarget::Sdk | RegenTarget::All) {
                sdk::regen()?;
            }
        }
        Cmd::Runtime => run::gradle(&[":reseam-sdk:patchRuntimeJar"])?,
        Cmd::PackSdk => {
            boltffi::check_version()?;
            sdk::pack()?;
        }
        Cmd::Release { version } => release::release(&version)?,
        Cmd::CheckTag { tag } => release::check_tag(&tag)?,
        Cmd::FrameworkAttributes { android_jar } => framework::generate(&android_jar)?,
    }
    Ok(())
}
