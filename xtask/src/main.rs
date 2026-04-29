// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

mod ndk;
mod patch_api;
mod paths;
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
    Regen {
        #[arg(value_enum)]
        target: RegenTarget,
    },
    JniHost {
        #[arg(long = "crate", value_enum, default_value_t = JniCrate::Patcher)]
        which: JniCrate,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum RegenTarget {
    PatchApi,
    Sdk,
    All,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum JniCrate {
    Patcher,
    Sdk,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Regen { target } => match target {
            RegenTarget::PatchApi => patch_api::regen()?,
            RegenTarget::Sdk => sdk::regen()?,
            RegenTarget::All => {
                patch_api::regen()?;
                sdk::regen()?;
            }
        },
        Cmd::JniHost { which } => match which {
            JniCrate::Patcher => patch_api::build_jni_host()?,
            JniCrate::Sdk => sdk::build_jni_host()?,
        },
    }
    Ok(())
}
