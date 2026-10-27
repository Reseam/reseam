// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

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

use crate::app::{BundleCommands, Cli, Commands, ReleaseCommands};
use crate::commands::{
    run_bundle_fetch, run_bundle_keygen, run_bundle_list, run_bundle_manifest, run_bundle_pack,
    run_bundle_pubkey, run_info, run_patch, run_perf, run_perf_worker, run_release_files,
    run_release_manager, run_release_patches,
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
            BundleCommands::Pubkey(command) => run_bundle_pubkey(&command),
            BundleCommands::Pack(command) => run_bundle_pack(&command),
            BundleCommands::Manifest(command) => run_bundle_manifest(&command),
            BundleCommands::List(command) => run_bundle_list(&command),
            BundleCommands::Fetch(command) => run_bundle_fetch(&command),
        },
        Commands::Release { command } => match command {
            ReleaseCommands::Patches(command) => run_release_patches(&command),
            ReleaseCommands::Manager(command) => run_release_manager(&command),
            ReleaseCommands::Files(command) => run_release_files(&command),
        },
    }
}
