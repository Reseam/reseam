// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "reseam", about = "High-performance APK patching engine")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Patch(PatchCommand),
    Perf(PerfCommand),
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

#[derive(Args, Clone)]
pub struct PatchRequestArgs {
    pub apk: PathBuf,
    #[arg(long = "split")]
    pub split: Vec<PathBuf>,
    #[arg(long)]
    pub bundle: PathBuf,
    #[arg(long, requires = "cert")]
    pub key: Option<PathBuf>,
    #[arg(long, requires = "key")]
    pub cert: Option<PathBuf>,
    #[arg(long = "enable")]
    pub enable: Vec<String>,
    #[arg(long = "disable")]
    pub disable: Vec<String>,
    #[arg(long = "option", value_name = "PATCH.KEY=VALUE")]
    pub option: Vec<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct PatchCommand {
    #[command(flatten)]
    pub request: PatchRequestArgs,
    #[arg(long, conflicts_with = "output_dir")]
    pub output: Option<PathBuf>,
    #[arg(long, conflicts_with = "output")]
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
}

#[derive(Subcommand)]
pub enum PublishCommands {
    Patches(PublishPatchesCommand),
}

#[derive(Args)]
pub struct PublishPatchesCommand {
    pub bundle: PathBuf,
    #[arg(long)]
    pub version: String,
    #[arg(long)]
    pub url: String,
    #[arg(long, default_value = "patches.json")]
    pub out: PathBuf,
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
