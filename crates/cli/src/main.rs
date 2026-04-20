// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reseam_library::{
    built_in_trust_store, inspect_apk as inspect_apk_with_library, inspect_with_trust,
    load_bundle_with_trust, patch as patch_with_library, selection_from_cli, ArtifactKind,
    PatchOutput as LibraryPatchOutput, PatchRequest, RunEvent,
};
use reseam_patcher::engine::PatchStatus;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "reseam", about = "High-performance APK patching engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Patch {
        apk: PathBuf,
        #[arg(long = "split")]
        split: Vec<PathBuf>,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,
        #[arg(long, conflicts_with = "output")]
        output_dir: Option<PathBuf>,
        #[arg(long, requires = "cert")]
        key: Option<PathBuf>,
        #[arg(long, requires = "key")]
        cert: Option<PathBuf>,
        #[arg(long = "enable")]
        enable: Vec<String>,
        #[arg(long = "disable")]
        disable: Vec<String>,
        #[arg(long = "option", value_name = "PATCH.KEY=VALUE")]
        option: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    Info {
        apk: PathBuf,
    },
    Bundle {
        #[command(subcommand)]
        command: BundleCommands,
    },
    Publish {
        #[command(subcommand)]
        command: PublishCommands,
    },
}

#[derive(Subcommand)]
enum BundleCommands {
    Keygen {
        #[arg(long)]
        out: PathBuf,
    },
    Pack {
        dir: PathBuf,
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    List {
        bundle: PathBuf,
    },
}

#[derive(Subcommand)]
enum PublishCommands {
    Patches {
        bundle: PathBuf,
        #[arg(long)]
        version: String,
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "patches.json")]
        out: PathBuf,
        #[arg(long, conflicts_with = "description_file")]
        description: Option<String>,
        #[arg(long)]
        description_file: Option<PathBuf>,
        #[arg(long)]
        homepage: Option<String>,
        #[arg(long)]
        created_at: Option<String>,
        #[arg(long)]
        prerelease: bool,
    },
}

fn main() -> Result<()> {
    init_logging()?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Patch {
            apk,
            split,
            bundle,
            output,
            output_dir,
            key,
            cert,
            enable,
            disable,
            option,
            dry_run,
        } => cmd_patch(PatchCommandArgs {
            apk_path: &apk,
            split_paths: &split,
            bundle_path: &bundle,
            output: output.as_deref(),
            output_dir: output_dir.as_deref(),
            key_path: key.as_deref(),
            cert_path: cert.as_deref(),
            enabled_patches: &enable,
            disabled_patches: &disable,
            option_args: &option,
            dry_run,
        }),
        Commands::Info { apk } => cmd_info(&apk),
        Commands::Bundle { command } => match command {
            BundleCommands::Keygen { out } => cmd_bundle_keygen(&out),
            BundleCommands::Pack { dir, key, out } => cmd_bundle_pack(&dir, &key, &out),
            BundleCommands::List { bundle } => cmd_list(&bundle),
        },
        Commands::Publish { command } => match command {
            PublishCommands::Patches {
                bundle,
                version,
                url,
                out,
                description,
                description_file,
                homepage,
                created_at,
                prerelease,
            } => cmd_publish_patches(PublishPatchesArgs {
                bundle: &bundle,
                version: &version,
                url: &url,
                out: &out,
                description: description.as_deref(),
                description_file: description_file.as_deref(),
                homepage: homepage.as_deref(),
                created_at: created_at.as_deref(),
                prerelease,
            }),
        },
    }
}

fn init_logging() -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "reseam=info,reseam_cli=info,reseam_patcher=info,reseam_apk=info,reseam_sign=info",
        )
    });

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .try_init()
        .map_err(|e| anyhow::anyhow!("failed to initialize logging: {e}"))
}

struct PatchCommandArgs<'a> {
    apk_path: &'a Path,
    split_paths: &'a [PathBuf],
    bundle_path: &'a Path,
    output: Option<&'a Path>,
    output_dir: Option<&'a Path>,
    key_path: Option<&'a Path>,
    cert_path: Option<&'a Path>,
    enabled_patches: &'a [String],
    disabled_patches: &'a [String],
    option_args: &'a [String],
    dry_run: bool,
}

fn cmd_patch(args: PatchCommandArgs<'_>) -> Result<()> {
    let split_mode = !args.split_paths.is_empty();
    if split_mode && args.output.is_some() {
        bail!("--output cannot be used with --split; use --output-dir instead");
    }
    if !split_mode && args.output_dir.is_some() {
        bail!("--output-dir can only be used with --split");
    }

    let output_target = if split_mode {
        let dir = match args.output_dir {
            Some(dir) => dir.to_path_buf(),
            None => {
                let stem = args
                    .apk_path
                    .file_stem()
                    .context("invalid APK path")?
                    .to_string_lossy();
                args.apk_path.with_file_name(format!("{stem}-patched"))
            }
        };
        PatchOutput::SplitDir(dir)
    } else {
        let path = match args.output {
            Some(p) => p.to_path_buf(),
            None => {
                let stem = args
                    .apk_path
                    .file_stem()
                    .context("invalid APK path")?
                    .to_string_lossy();
                args.apk_path.with_file_name(format!("{stem}-patched.apk"))
            }
        };
        PatchOutput::SingleFile(path)
    };

    let trust_store = built_in_trust_store();
    let patch_bundle = load_bundle_with_trust(args.bundle_path, &trust_store)?;
    let selection =
        selection_from_cli(
            args.enabled_patches,
            args.disabled_patches,
            args.option_args,
            &patch_bundle,
        )?;
    let request = PatchRequest {
        apk_path: args.apk_path.to_path_buf(),
        split_paths: args.split_paths.to_vec(),
        bundle_paths: vec![args.bundle_path.to_path_buf()],
        trust_store,
        selection,
        output: match output_target {
            PatchOutput::SingleFile(path) => LibraryPatchOutput::SingleFile(path),
            PatchOutput::SplitDir(path) => LibraryPatchOutput::SplitDir(path),
        },
        key_path: args.key_path.map(Path::to_path_buf),
        cert_path: args.cert_path.map(Path::to_path_buf),
        dry_run: args.dry_run,
    };

    let outcome = patch_with_library(&request, |event| match event {
        RunEvent::Info { message } => info!(message),
        RunEvent::PatchStarted { patch } => info!(patch, "patch started"),
        RunEvent::PatchFinished {
            patch,
            status,
            reason,
        } => match status {
            reseam_library::PatchRunStatus::Applied => info!(patch, "patch completed"),
            reseam_library::PatchRunStatus::Skipped => {
                warn!(patch, reason = reason.unwrap_or_default(), "patch skipped")
            }
            reseam_library::PatchRunStatus::Failed => {
                error!(patch, reason = reason.unwrap_or_default(), "patch failed")
            }
        },
        RunEvent::PatchLog {
            patch,
            level,
            message,
        } => info!(patch, level, message, "patch log"),
    })?;

    let failed_count = outcome
        .results
        .iter()
        .filter(|result| matches!(result.status, PatchStatus::Failed { .. }))
        .count();

    if args.dry_run {
        if failed_count > 0 {
            bail!("{failed_count} patch(es) failed validation");
        }
        info!("dry run enabled; validation completed without applying patches");
        return Ok(());
    }

    if let Some(artifact) = outcome.artifact {
        match artifact.kind {
            ArtifactKind::Apk => info!(path = %artifact.path.display(), "patched APK ready"),
            ArtifactKind::SplitDirectory => {
                info!(path = %artifact.path.display(), "patched split APK set ready")
            }
        }
    }

    Ok(())
}

fn cmd_list(bundle_path: &Path) -> Result<()> {
    let response = inspect_with_trust(
        &[bundle_path.to_path_buf()],
        None,
        &[],
        &built_in_trust_store(),
    )?;
    let metadata = response
        .bundles
        .into_iter()
        .next()
        .context("bundle metadata missing from inspect response")?;
    println!("bundle: {}", metadata.name);
    if !metadata.author.is_empty() {
        println!("author: {}", metadata.author);
    }
    if !metadata.description.is_empty() {
        println!("description: {}", metadata.description);
    }
    println!();
    for (i, patch) in response.patches.iter().enumerate() {
        let enabled = if patch.enabled_by_default { "on" } else { "off" };
        println!(
            "  {:>3}. [{}] {} - {}",
            i + 1,
            enabled,
            patch.name,
            patch.description
        );

        if !patch.compatible_with.is_empty() {
            let formatted: Vec<String> = patch
                .compatible_with
                .iter()
                .map(|c| {
                    if c.versions.is_empty() {
                        c.package_name.clone()
                    } else {
                        format!("{} ({})", c.package_name, c.versions.join(", "))
                    }
                })
                .collect();
            println!("       packages: {}", formatted.join(", "));
        }

        if !patch.dependencies.is_empty() {
            println!("       depends: {}", patch.dependencies.join(", "));
        }

        if !patch.options.is_empty() {
            println!("       options:");
            for option in &patch.options {
                let required = if option.required {
                    "required"
                } else {
                    "optional"
                };
                println!(
                    "         - {} ({:?}, {})",
                    option.key, option.option_type, required
                );
            }
        }
    }

    if !metadata.extension_dex.is_empty() {
        println!();
        println!("extension DEX:");
        for dex_path in &metadata.extension_dex {
            println!("  - {}", dex_path);
        }
    }

    Ok(())
}

fn cmd_info(apk_path: &Path) -> Result<()> {
    let apk = inspect_apk_with_library(apk_path, &[])?;

    println!("APK: {}", apk_path.display());
    if let Some(pkg) = apk.package_name {
        println!("  package:    {pkg}");
    }
    if let Some(ver) = apk.version_name {
        println!("  version:    {ver}");
    }
    if let Some(code) = apk.version_code {
        println!("  versionCode: {code}");
    }
    println!("  dex files:  {}", apk.dex_files);
    println!("  components: {}", apk.component_count);
    if !apk.split_names.is_empty() {
        println!("  splits:     {}", apk.split_names.join(", "));
    }
    println!("  classes:    {}", apk.class_count);
    println!("  methods:    {}", apk.method_count);

    Ok(())
}

struct PublishPatchesArgs<'a> {
    bundle: &'a Path,
    version: &'a str,
    url: &'a str,
    out: &'a Path,
    description: Option<&'a str>,
    description_file: Option<&'a Path>,
    homepage: Option<&'a str>,
    created_at: Option<&'a str>,
    prerelease: bool,
}

#[derive(Debug)]
struct BundleArchiveInfo {
    name: String,
    author: String,
    description: String,
    public_key: String,
}

#[derive(Debug, Deserialize)]
struct BundleIndexManifest {
    bundle: BundleIndexManifestInfo,
}

#[derive(Debug, Deserialize)]
struct BundleIndexManifestInfo {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PatchesIndex {
    bundle: PatchesIndexBundle,
    releases: Vec<PatchesIndexRelease>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PatchesIndexBundle {
    name: String,
    author: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
    public_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PatchesIndexRelease {
    version: String,
    created_at: String,
    description: String,
    download_url: String,
    prerelease: bool,
}

fn cmd_publish_patches(args: PublishPatchesArgs<'_>) -> Result<()> {
    if args.version.trim().is_empty() {
        bail!("--version must not be empty");
    }
    if args.url.trim().is_empty() {
        bail!("--url must not be empty");
    }

    let archive = inspect_reseam_archive(args.bundle)
        .with_context(|| format!("failed to inspect bundle archive {}", args.bundle.display()))?;

    let release_description = match (args.description, args.description_file) {
        (Some(text), None) => text.to_owned(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?,
        (None, None) => String::new(),
        (Some(_), Some(_)) => bail!("--description and --description-file are mutually exclusive"),
    };

    let created_at = match args.created_at {
        Some(value) => {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
                .context("--created-at must be an RFC3339 timestamp")?;
            value.to_owned()
        }
        None => time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .context("failed to format current time")?,
    };

    let existing = if args.out.exists() {
        let json = std::fs::read_to_string(args.out)
            .with_context(|| format!("failed to read {}", args.out.display()))?;
        Some(
            serde_json::from_str::<PatchesIndex>(&json)
                .with_context(|| format!("failed to parse {}", args.out.display()))?,
        )
    } else {
        None
    };

    if let Some(existing) = &existing {
        if existing.bundle.public_key != archive.public_key {
            bail!(
                "refusing to change bundle public key in {} (existing {}, archive {})",
                args.out.display(),
                existing.bundle.public_key,
                archive.public_key
            );
        }
    }

    let homepage = args.homepage.map(str::to_owned).or_else(|| {
        existing
            .as_ref()
            .and_then(|index| index.bundle.homepage.clone())
    });

    let mut releases = existing
        .map(|mut index| {
            index
                .releases
                .retain(|release| release.version != args.version);
            index.releases
        })
        .unwrap_or_default();

    releases.insert(
        0,
        PatchesIndexRelease {
            version: args.version.to_owned(),
            created_at,
            description: release_description,
            download_url: args.url.to_owned(),
            prerelease: args.prerelease,
        },
    );

    let index = PatchesIndex {
        bundle: PatchesIndexBundle {
            name: archive.name,
            author: archive.author,
            description: archive.description,
            homepage,
            public_key: archive.public_key,
        },
        releases,
    };

    write_json_atomically(args.out, &index)?;
    info!(out = %args.out.display(), "patches index written");
    Ok(())
}

fn inspect_reseam_archive(path: &Path) -> Result<BundleArchiveInfo> {
    let file =
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("failed to open zip {}", path.display()))?;

    let mimetype = read_zip_entry(&mut archive, "mimetype")?;
    if mimetype != reseam_patcher::bundle::BUNDLE_MIMETYPE.as_bytes() {
        bail!(
            "invalid mimetype marker (expected {})",
            reseam_patcher::bundle::BUNDLE_MIMETYPE
        );
    }

    let manifest_bytes = read_zip_entry(&mut archive, "manifest.toml")?;
    let pubkey_bytes = read_zip_entry(&mut archive, "manifest.pubkey")?;
    let sig_bytes = read_zip_entry(&mut archive, "manifest.sig")?;

    let pubkey_arr: [u8; 32] = pubkey_bytes.as_slice().try_into().with_context(|| {
        format!(
            "manifest.pubkey has wrong length: {} (expected 32)",
            pubkey_bytes.len()
        )
    })?;
    let verifying_key =
        VerifyingKey::from_bytes(&pubkey_arr).context("invalid Ed25519 public key")?;
    let signature = Signature::from_slice(&sig_bytes).context("invalid Ed25519 signature")?;
    verifying_key
        .verify(&manifest_bytes, &signature)
        .context("manifest signature verification failed")?;

    let manifest: BundleIndexManifest =
        toml::from_str(std::str::from_utf8(&manifest_bytes).context("manifest.toml is not UTF-8")?)
            .context("failed to parse manifest.toml")?;

    Ok(BundleArchiveInfo {
        name: manifest.bundle.name,
        author: manifest.bundle.author,
        description: manifest.bundle.description,
        public_key: hex::encode(pubkey_arr),
    })
}

fn read_zip_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .with_context(|| format!("missing required entry `{name}`"))?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read `{name}`"))?;
    Ok(bytes)
}

fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("invalid output path {}", path.display()))?;
    let tmp_name = format!(".{file_name}.{}.tmp", std::process::id());
    let tmp_path = parent.unwrap_or_else(|| Path::new(".")).join(tmp_name);

    let json = serde_json::to_vec_pretty(value).context("failed to serialize patches index")?;
    std::fs::write(&tmp_path, [&json[..], b"\n"].concat())
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to move {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

enum PatchOutput {
    SingleFile(PathBuf),
    SplitDir(PathBuf),
}

fn cmd_bundle_keygen(out: &Path) -> Result<()> {
    use rand::RngCore;
    use std::os::unix::fs::OpenOptionsExt;

    if out.exists() {
        bail!("refusing to overwrite existing key at {}", out.display());
    }
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    }

    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let pubkey = signing_key.verifying_key().to_bytes();

    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(out)
        .with_context(|| format!("failed to create {}", out.display()))?;
    use std::io::Write as _;
    file.write_all(&seed).context("failed to write seed")?;

    println!("Ed25519 keypair generated");
    println!("  private seed: {}", out.display());
    println!("  public key (hex): {}", hex::encode(pubkey));
    println!("  trust this signer in your client before loading its bundles");
    Ok(())
}

fn cmd_bundle_pack(dir: &Path, key_path: &Path, out_path: &Path) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::Write as _;

    let manifest_path = dir.join("manifest.toml");
    let manifest_src = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;

    #[derive(serde::Deserialize)]
    struct PartialManifest {
        bundle: PartialBundle,
    }
    #[derive(serde::Deserialize)]
    struct PartialBundle {
        name: String,
        #[serde(default)]
        author: String,
        #[serde(default)]
        description: String,
        format_version: u32,
    }

    let partial: PartialManifest =
        toml::from_str(&manifest_src).context("failed to parse manifest.toml")?;
    if partial.bundle.format_version != reseam_patcher::bundle::BUNDLE_FORMAT_VERSION {
        bail!(
            "unsupported format_version {}; CLI supports {}",
            partial.bundle.format_version,
            reseam_patcher::bundle::BUNDLE_FORMAT_VERSION
        );
    }

    let mut payload: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("non-utf8 filename")?
            .to_string();
        if name == "manifest.toml" {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".jar") || lower.ends_with(".dex") || lower.ends_with(".rve")) {
            continue;
        }
        let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        payload.push((name, bytes));
    }
    payload.sort_by(|a, b| a.0.cmp(&b.0));

    if payload.is_empty() {
        bail!("no .jar/.dex/.rve files found in {}", dir.display());
    }

    let mut manifest = String::new();
    manifest.push_str("[bundle]\n");
    manifest.push_str(&format!("name = {}\n", toml_string(&partial.bundle.name)));
    if !partial.bundle.author.is_empty() {
        manifest.push_str(&format!(
            "author = {}\n",
            toml_string(&partial.bundle.author)
        ));
    }
    if !partial.bundle.description.is_empty() {
        manifest.push_str(&format!(
            "description = {}\n",
            toml_string(&partial.bundle.description)
        ));
    }
    manifest.push_str(&format!(
        "format_version = {}\n\n",
        partial.bundle.format_version
    ));
    manifest.push_str("[files]\n");
    for (name, bytes) in &payload {
        manifest.push_str(&format!(
            "{} = \"{}\"\n",
            toml_string(name),
            hex::encode(Sha256::digest(bytes))
        ));
    }
    let manifest_bytes = manifest.into_bytes();

    let seed =
        std::fs::read(key_path).with_context(|| format!("read key {}", key_path.display()))?;
    if seed.len() != 32 {
        bail!("signing key must be exactly 32 bytes, got {}", seed.len());
    }
    let seed: [u8; 32] = seed
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("signing key must be exactly 32 bytes"))?;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let pubkey = signing_key.verifying_key().to_bytes();
    let signature = ed25519_dalek::Signer::sign(&signing_key, &manifest_bytes).to_bytes();

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file = std::fs::File::create(out_path)
        .with_context(|| format!("create {}", out_path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("mimetype", stored)?;
    zip.write_all(reseam_patcher::bundle::BUNDLE_MIMETYPE.as_bytes())?;
    zip.start_file("manifest.toml", deflated)?;
    zip.write_all(&manifest_bytes)?;
    zip.start_file("manifest.pubkey", stored)?;
    zip.write_all(&pubkey)?;
    zip.start_file("manifest.sig", stored)?;
    zip.write_all(&signature)?;
    for (name, bytes) in &payload {
        zip.start_file(name, deflated)?;
        zip.write_all(bytes)?;
    }
    zip.finish()?;

    info!(
        bundle = %partial.bundle.name,
        out = %out_path.display(),
        file_count = payload.len(),
        "bundle packed and signed"
    );
    Ok(())
}

fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
