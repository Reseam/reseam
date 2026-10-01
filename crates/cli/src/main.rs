// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI commands print results and progress to their standard streams"
)]

mod app;
mod commands;
mod logging;

use anyhow::Result;
use clap::Parser;

use crate::app::{BundleCommands, Cli, Commands, PublishCommands};
use crate::commands::{
    run_bundle_keygen, run_bundle_list, run_bundle_pack, run_info, run_patch, run_perf,
    run_perf_worker, run_publish_manager, run_publish_patches,
};
use crate::logging::init_logging;

fn main() -> Result<()> {
    init_logging()?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Patch(command) => run_patch(&command),
        Commands::Perf(command) => run_perf(&command),
        Commands::PerfWorker => run_perf_worker(),
        Commands::Info(command) => run_info(&command),
        Commands::Bundle { command } => match command {
            BundleCommands::Keygen(command) => run_bundle_keygen(&command),
            BundleCommands::Pack(command) => run_bundle_pack(&command),
            BundleCommands::Manifest { path } => {
                println!(
                    "{}",
                    serde_json::to_string(&reseam_patcher::bundle::manifest_info(&path)?)?
                );
                Ok(())
            }
            BundleCommands::List(command) => run_bundle_list(&command),
        },
        Commands::Publish { command } => match command {
            PublishCommands::Patches(command) => run_publish_patches(&command),
            PublishCommands::Manager(command) => run_publish_manager(&command),
        },
    }
}
