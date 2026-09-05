// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

mod jni;
mod ndk;
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
    /// Regenerates the BoltFFI Kotlin bindings, and for the sdk the Android jniLibs.
    Regen {
        #[arg(value_enum)]
        target: RegenTarget,
    },
    /// Builds the desktop JNI shim the JVM sdk loads.
    JniHost,
    /// Sets the workspace version, commits, and tags the release.
    Release { version: String },
    /// Fails unless the tag names the workspace version.
    CheckTag { tag: String },
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
            if matches!(target, RegenTarget::PatchApi | RegenTarget::All) {
                patch_api::regen()?;
            }
            if matches!(target, RegenTarget::Sdk | RegenTarget::All) {
                sdk::regen()?;
            }
        }
        Cmd::JniHost => sdk::build_jni_host()?,
        Cmd::Release { version } => release::release(&version)?,
        Cmd::CheckTag { tag } => release::check_tag(&tag)?,
    }
    Ok(())
}
