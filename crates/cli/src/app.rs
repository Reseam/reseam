// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Args, Parser, Subcommand};
use reseam_model::PatchPreset;
use url::Url;

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
    /// Publishes a release to a GitHub or Forgejo release, an S3 bucket or a folder.
    Release {
        #[command(subcommand)]
        command: ReleaseCommands,
    },
}

#[derive(Args)]
pub struct TrustArgs {
    #[arg(long = "trust", value_name = "PUBLIC_KEY_HEX")]
    pub trust: Vec<String>,
}

#[derive(Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent command-line switches"
)]
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
    /// Write the output without patches that fail, and the patches that need them.
    #[arg(long)]
    pub leave_out_failed: bool,
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
    /// Print the public key of a signing key made by `keygen`.
    Pubkey(BundlePubkeyCommand),
    Pack(BundlePackCommand),
    /// Validate a staging manifest and print its bundle metadata as JSON.
    Manifest(BundleManifestCommand),
    List(BundleListCommand),
    /// Download a release from a published index and verify it against the index signer.
    Fetch(BundleFetchCommand),
}

#[derive(Args)]
pub struct BundleKeygenCommand {
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Args)]
pub struct BundlePubkeyCommand {
    #[arg(long)]
    pub key: PathBuf,
}

#[derive(Args)]
pub struct BundleManifestCommand {
    pub path: PathBuf,
}

#[derive(Args)]
pub struct BundlePackCommand {
    pub dir: PathBuf,
    /// The release this bundle is; omit it for development builds.
    #[arg(long)]
    pub version: Option<String>,
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

#[derive(Args)]
pub struct BundleFetchCommand {
    /// The release index (`patches.json`) URL; `file:` URLs read from disk.
    #[arg(long)]
    pub index: String,
    #[arg(long)]
    pub version: String,
    /// Fail unless the index names this public key.
    #[arg(long, value_name = "PUBLIC_KEY_HEX")]
    pub signer: Option<String>,
    /// The folder that keeps fetched bundles; a verified copy already there is reused.
    #[arg(long, default_value = ".")]
    pub to: PathBuf,
}

#[derive(Subcommand)]
pub enum ReleaseCommands {
    /// A signed bundle, added to the published patches.json.
    Patches(ReleasePatchesCommand),
    /// The Reseam Manager builds in a folder, added to the published manager.json.
    Manager(ReleaseManagerCommand),
    /// The files in a folder.
    Files(ReleaseFilesCommand),
}

#[derive(Args)]
pub struct ReleaseArgs {
    /// The tag the release is for.
    #[arg(long)]
    pub tag: String,
    /// `forge` for the release of the GitHub or Forgejo workflow running this, `s3://<bucket>/<prefix>`,
    /// or a folder.
    #[arg(long, default_value = "forge")]
    pub to: Target,
    /// The URL an s3 or folder target is served from, for the links in the index.
    #[arg(long)]
    pub public_url: Option<Url>,
    /// Also publish a GitHub or Forgejo release with the notes, for s3 and folder targets.
    #[arg(long)]
    pub forge_release: bool,
    /// The release notes; the subjects of the commits since the previous tag by default.
    #[arg(long)]
    pub notes: Option<PathBuf>,
    /// Keep only the newest releases in an s3 or folder target.
    #[arg(long)]
    pub keep: Option<NonZeroUsize>,
}

#[derive(Clone)]
pub enum Target {
    Forge,
    S3 { bucket: String, prefix: String },
    Directory(PathBuf),
}

impl FromStr for Target {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "forge" {
            return Ok(Self::Forge);
        }
        let Some(location) = value.strip_prefix("s3://") else {
            return Ok(Self::Directory(PathBuf::from(value)));
        };
        let (bucket, prefix) = location.split_once('/').unwrap_or((location, ""));
        if bucket.is_empty() {
            return Err("an s3 target names a bucket: s3://<bucket>/<prefix>".into());
        }
        Ok(Self::S3 {
            bucket: bucket.to_owned(),
            prefix: prefix.trim_matches('/').to_owned(),
        })
    }
}

#[derive(Args)]
pub struct ReleasePatchesCommand {
    pub bundle: PathBuf,
    #[command(flatten)]
    pub release: ReleaseArgs,
    /// The publisher's page; kept from the published index, or the repository page by default.
    #[arg(long)]
    pub homepage: Option<String>,
}

#[derive(Args)]
pub struct ReleaseManagerCommand {
    pub dir: PathBuf,
    #[command(flatten)]
    pub release: ReleaseArgs,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub author: String,
    #[arg(long, default_value = "")]
    pub summary: String,
    /// Where users download this release.
    #[arg(long)]
    pub download_url: Url,
}

#[derive(Args)]
pub struct ReleaseFilesCommand {
    pub dir: PathBuf,
    #[command(flatten)]
    pub release: ReleaseArgs,
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
