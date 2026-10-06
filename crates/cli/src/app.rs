// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use reseam_patcher::PatchPreset;

#[derive(Parser)]
#[command(name = "reseam", version, about = "APK patching engine")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Patch(PatchCommand),
    Perf(PerfCommand),
    #[command(hide = true)]
    PerfWorker,
    Info(InfoCommand),
    Bundle {
        #[command(subcommand)]
        command: BundleCommands,
    },
    Publish {
        #[command(subcommand)]
        command: PublishCommands,
    },
}

#[derive(Args)]
pub struct TrustArgs {
    #[arg(long = "trust", value_name = "PUBLIC_KEY_HEX")]
    pub trust: Vec<String>,
}

#[derive(Args)]
pub struct PatchRequestArgs {
    pub apk: PathBuf,
    #[arg(long = "split")]
    pub split: Vec<PathBuf>,
    /// Repeatable; a patch may depend on one from another loaded bundle.
    #[arg(long = "bundle", required = true)]
    pub bundle: Vec<PathBuf>,
    #[command(flatten)]
    pub trust: TrustArgs,
    #[arg(long, requires = "cert")]
    pub key: Option<PathBuf>,
    #[arg(long, requires = "key")]
    pub cert: Option<PathBuf>,
    /// Patches to start from: recommended, all or none. `--enable` and `--disable` adjust it.
    #[arg(long, default_value = "recommended")]
    pub preset: PatchPreset,
    #[arg(long = "enable")]
    pub enable: Vec<String>,
    #[arg(long = "disable")]
    pub disable: Vec<String>,
    #[arg(long = "option", value_name = "PATCH.KEY=VALUE")]
    pub option: Vec<String>,
    #[arg(long)]
    pub dry_run: bool,
    /// Run patches on app versions they were not declared for.
    #[arg(long)]
    pub ignore_versions: bool,
    /// Build output to mount over the installed app, leaving out patches that change the manifest.
    #[arg(long)]
    pub mount: bool,
}

#[derive(Args)]
pub struct PatchCommand {
    #[command(flatten)]
    pub request: PatchRequestArgs,
    /// File for a single-component input.
    #[arg(long, conflicts_with_all = ["split", "output_dir"])]
    pub output: Option<PathBuf>,
    /// Directory for patched APK components, including a single APK.
    #[arg(long)]
    pub output_dir: Option<PathBuf>,
}

#[derive(Args)]
pub struct PerfCommand {
    #[command(flatten)]
    pub request: PatchRequestArgs,
    #[arg(long, default_value_t = 1)]
    pub iterations: u32,
    #[arg(long, default_value_t = 0)]
    pub warmup: u32,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct InfoCommand {
    pub apk: PathBuf,
}

#[derive(Subcommand)]
pub enum BundleCommands {
    Keygen(BundleKeygenCommand),
    Pack(BundlePackCommand),
    /// Validate a staging manifest and print its bundle metadata as JSON.
    Manifest {
        path: PathBuf,
    },
    List(BundleListCommand),
}

#[derive(Args)]
pub struct BundleKeygenCommand {
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Args)]
pub struct BundlePackCommand {
    pub dir: PathBuf,
    #[arg(long)]
    pub key: PathBuf,
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Args)]
pub struct BundleListCommand {
    pub bundle: PathBuf,
    #[command(flatten)]
    pub trust: TrustArgs,
    /// Print the inspect response as JSON instead of the listing.
    #[arg(long)]
    pub json: bool,
    /// Include payload filenames and the packing engine version.
    #[arg(long, conflicts_with = "json")]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum PublishCommands {
    Patches(PublishPatchesCommand),
    Manager(PublishManagerCommand),
}

#[derive(Args)]
pub struct ReleaseArgs {
    #[arg(long)]
    pub version: String,
    #[arg(long)]
    pub url: String,
    #[arg(long, conflicts_with = "description_file")]
    pub description: Option<String>,
    #[arg(long)]
    pub description_file: Option<PathBuf>,
    #[arg(long)]
    pub homepage: Option<String>,
    #[arg(long)]
    pub created_at: Option<String>,
    #[arg(long)]
    pub prerelease: bool,
}

#[derive(Args)]
pub struct PublishPatchesCommand {
    pub bundle: PathBuf,
    #[command(flatten)]
    pub release: ReleaseArgs,
    #[arg(long, default_value = "patches.json")]
    pub out: PathBuf,
}

#[derive(Args)]
pub struct PublishManagerCommand {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub author: String,
    #[arg(long, default_value = "")]
    pub summary: String,
    #[command(flatten)]
    pub release: ReleaseArgs,
    #[arg(long, default_value = "manager.json")]
    pub out: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_arguments_enforce_output_and_signing_constraints() {
        let prefix = ["reseam", "patch", "app.apk", "--bundle", "patches.reseam"];
        for flags in [
            vec!["--output", "out.apk", "--output-dir", "out"],
            vec!["--output", "out.apk", "--split", "config.en.apk"],
            vec!["--key", "identity.pk8"],
            vec!["--cert", "identity.der"],
        ] {
            assert!(Cli::try_parse_from(prefix.into_iter().chain(flags)).is_err());
        }
        for flags in [
            vec!["--output", "out.apk"],
            vec!["--output-dir", "out", "--split", "config.en.apk"],
            vec!["--key", "identity.pk8", "--cert", "identity.der"],
        ] {
            assert!(Cli::try_parse_from(prefix.into_iter().chain(flags)).is_ok());
        }
    }
}
