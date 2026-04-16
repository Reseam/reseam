// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reseam_apk::{ApkFile, ApkWriteOptions};
use reseam_patcher::bundle::PatchBundle;
use reseam_patcher::context::PatchContext;
use reseam_patcher::engine::{self, ExecutionPlan, PatchStatus};
use reseam_patcher::options::{OptionDeclaration, PatchOptions};
use reseam_sign::{GeneratedKey, SigningKey};
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
        } => cmd_patch(
            &apk,
            &split,
            &bundle,
            output.as_deref(),
            output_dir.as_deref(),
            key.as_deref(),
            cert.as_deref(),
            &enable,
            &disable,
            &option,
            dry_run,
        ),
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

fn cmd_patch(
    apk_path: &Path,
    split_paths: &[PathBuf],
    bundle_path: &Path,
    output: Option<&Path>,
    output_dir: Option<&Path>,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
    enabled_patches: &[String],
    disabled_patches: &[String],
    option_args: &[String],
    dry_run: bool,
) -> Result<()> {
    let split_mode = !split_paths.is_empty();
    if split_mode && output.is_some() {
        bail!("--output cannot be used with --split; use --output-dir instead");
    }
    if !split_mode && output_dir.is_some() {
        bail!("--output-dir can only be used with --split");
    }

    let output_target = if split_mode {
        let dir = match output_dir {
            Some(dir) => dir.to_path_buf(),
            None => {
                let stem = apk_path
                    .file_stem()
                    .context("invalid APK path")?
                    .to_string_lossy();
                apk_path.with_file_name(format!("{stem}-patched"))
            }
        };
        PatchOutput::SplitDir(dir)
    } else {
        let path = match output {
            Some(p) => p.to_path_buf(),
            None => {
                let stem = apk_path
                    .file_stem()
                    .context("invalid APK path")?
                    .to_string_lossy();
                apk_path.with_file_name(format!("{stem}-patched.apk"))
            }
        };
        PatchOutput::SingleFile(path)
    };

    info!(
        apk_path = %apk_path.display(),
        split_count = split_paths.len(),
        "opening APK"
    );
    let mut apk = if split_mode {
        ApkFile::open_split(apk_path, split_paths).context("failed to open split APK set")?
    } else {
        ApkFile::open(apk_path).context("failed to open APK")?
    };

    if apk.is_split() {
        info!(components = apk.component_count(), splits = ?apk.split_names(), "loaded split APK set");
    } else {
        info!("loaded single APK");
    }

    if let Some(pkg) = apk.package_name() {
        info!(package = pkg, "loaded APK package");
    }
    if let Some(ver) = apk.version_name() {
        info!(version = ver, "loaded APK version");
    }
    info!(dex_files = apk.dex().len(), "APK ready for patching");

    info!(bundle_path = %bundle_path.display(), "loading patch bundle");
    let patch_bundle = PatchBundle::load(bundle_path).context("failed to load patch bundle")?;
    info!(
        bundle = %patch_bundle.name,
        patch_count = patch_bundle.patches.len(),
        "patch bundle loaded"
    );
    let plan = build_execution_plan(
        &patch_bundle,
        enabled_patches,
        disabled_patches,
        option_args,
    )?;

    if dry_run {
        let results = engine::validate_patches_with_plan(
            &patch_bundle.patches,
            &plan,
            apk.package_name(),
            apk.version_name(),
        )
        .context("patch validation failed")?;

        log_patch_results(&results, "validated", "validation completed");
        let failed_count = results
            .iter()
            .filter(|r| matches!(r.status, PatchStatus::Failed { .. }))
            .count();
        if failed_count > 0 {
            bail!("{failed_count} patch(es) failed validation");
        }
        info!("dry run enabled; validation completed without applying patches");
        return Ok(());
    }

    let mut ctx = PatchContext::new(&mut apk);

    let results = engine::apply_patches_with_plan(&mut ctx, &patch_bundle.patches, &plan)
        .context("patch application failed")?;
    drop(ctx);

    let failed_count = log_patch_results(&results, "applied", "patch run completed");
    if failed_count > 0 {
        bail!("{failed_count} patch(es) failed");
    }

    match output_target {
        PatchOutput::SingleFile(output_path) => {
            write_signed_single_apk(&mut apk, &output_path, key_path, cert_path)
        }
        PatchOutput::SplitDir(output_dir) => {
            write_signed_split_apks(&mut apk, &output_dir, key_path, cert_path)
        }
    }
}

fn cmd_list(bundle_path: &Path) -> Result<()> {
    let bundle = PatchBundle::load(bundle_path).context("failed to load patch bundle")?;
    println!("bundle: {}", bundle.name);
    if !bundle.author.is_empty() {
        println!("author: {}", bundle.author);
    }
    if !bundle.description.is_empty() {
        println!("description: {}", bundle.description);
    }
    println!();
    for (i, patch) in bundle.patches.iter().enumerate() {
        let p: &dyn reseam_patcher::patch::Patch = patch.as_ref();
        let enabled = if p.enabled_by_default() { "on" } else { "off" };
        println!(
            "  {:>3}. [{}] {} - {}",
            i + 1,
            enabled,
            p.name(),
            p.description()
        );

        let compat = p.compatible_with();
        if !compat.is_empty() {
            let formatted: Vec<String> = compat
                .iter()
                .map(|c| {
                    if c.versions.is_empty() {
                        c.package.clone()
                    } else {
                        format!("{} ({})", c.package, c.versions.join(", "))
                    }
                })
                .collect();
            println!("       packages: {}", formatted.join(", "));
        }

        let deps = p.depends_on();
        if !deps.is_empty() {
            println!("       depends: {}", deps.join(", "));
        }

        let options = p.options();
        if !options.is_empty() {
            println!("       options:");
            for option in options {
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

    if !bundle.extension_dex.is_empty() {
        println!();
        println!("extension DEX:");
        for dex_path in &bundle.extension_dex {
            println!("  - {}", dex_path.display());
        }
    }

    Ok(())
}

fn cmd_info(apk_path: &Path) -> Result<()> {
    let apk = ApkFile::open(apk_path).context("failed to open APK")?;

    println!("APK: {}", apk_path.display());
    if let Some(pkg) = apk.package_name() {
        println!("  package:    {pkg}");
    }
    if let Some(ver) = apk.version_name() {
        println!("  version:    {ver}");
    }
    if let Some(code) = apk.version_code() {
        println!("  versionCode: {code}");
    }
    println!("  dex files:  {}", apk.dex().len());
    println!("  components: {}", apk.component_count());
    if apk.is_split() {
        println!("  splits:     {}", apk.split_names().join(", "));
    }

    let total_classes: usize = apk.dex().iter().map(|d| d.classes.len()).sum();
    let total_methods: usize = apk.dex().iter().map(|d| d.methods.len()).sum();
    println!("  classes:    {total_classes}");
    println!("  methods:    {total_methods}");

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

fn log_patch_results(
    results: &[engine::PatchResult],
    applied_verb: &str,
    complete_message: &str,
) -> usize {
    for result in results {
        match &result.status {
            PatchStatus::Applied => {
                info!(patch = %result.name, verb = applied_verb, "patch completed")
            }
            PatchStatus::Skipped { reason } => {
                warn!(patch = %result.name, reason, "patch skipped")
            }
            PatchStatus::Failed { reason } => {
                error!(patch = %result.name, reason, "patch failed")
            }
        }
        for log in &result.logs {
            info!(
                patch = %log.patch,
                level = %log.level,
                message = %log.message,
                "patch log"
            );
        }
    }

    let applied_count = results
        .iter()
        .filter(|r| matches!(r.status, PatchStatus::Applied))
        .count();
    let failed_count = results
        .iter()
        .filter(|r| matches!(r.status, PatchStatus::Failed { .. }))
        .count();
    info!(
        applied_count,
        total = results.len(),
        failed_count,
        summary = complete_message,
        "patch summary"
    );
    failed_count
}

fn write_signed_single_apk(
    apk: &mut ApkFile,
    output_path: &Path,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
) -> Result<()> {
    if let Some(parent) = output_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }

    info!(output_path = %output_path.display(), "writing patched APK");
    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    apk.write_to_with_options(
        tmp_dir.path(),
        ApkWriteOptions {
            strip_signatures: true,
        },
    )
    .context("failed to write patched APK")?;

    let tmp_apk_path = find_output_apks(tmp_dir.path())?
        .into_iter()
        .next()
        .context("no APK file found in output directory")?;
    let signing_key = load_or_generate_key(
        output_path.with_extension("pk8"),
        output_path.with_extension("der"),
        key_path,
        cert_path,
    )?;
    sign_apk_to_path(&tmp_apk_path, output_path, &signing_key)?;

    Ok(())
}

fn write_signed_split_apks(
    apk: &mut ApkFile,
    output_dir: &Path,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;

    info!(output_dir = %output_dir.display(), "writing patched split APK set");
    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    apk.write_to_with_options(
        tmp_dir.path(),
        ApkWriteOptions {
            strip_signatures: true,
        },
    )
    .context("failed to write patched split APK set")?;

    let signing_key = load_or_generate_key(
        output_dir.join("reseam.pk8"),
        output_dir.join("reseam.der"),
        key_path,
        cert_path,
    )?;

    for unsigned_apk in find_output_apks(tmp_dir.path())? {
        let file_name = unsigned_apk
            .file_name()
            .context("temporary APK output is missing a filename")?;
        let output_path = output_dir.join(file_name);
        sign_apk_to_path(&unsigned_apk, &output_path, &signing_key)?;
    }

    info!(output_dir = %output_dir.display(), "patched split APK set written");
    Ok(())
}

fn find_output_apks(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut apks = Vec::new();
    for entry in std::fs::read_dir(dir).context("failed to read temp directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "apk") {
            apks.push(path);
        }
    }
    apks.sort();
    Ok(apks)
}

fn sign_apk_to_path(
    unsigned_path: &Path,
    output_path: &Path,
    signing_key: &SigningKey,
) -> Result<()> {
    let unsigned_bytes = std::fs::read(unsigned_path)
        .with_context(|| format!("failed to read {}", unsigned_path.display()))?;
    info!(
        unsigned_path = %unsigned_path.display(),
        unsigned_size = unsigned_bytes.len(),
        "loaded patched APK bytes"
    );

    info!("signing APK with Signature Scheme v2");
    let signed_bytes =
        reseam_sign::v2::sign(&unsigned_bytes, signing_key).context("v2 signing failed")?;

    std::fs::write(output_path, &signed_bytes)
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    info!(
        output_path = %output_path.display(),
        size_mb = signed_bytes.len() as f64 / (1024.0 * 1024.0),
        "patched APK written"
    );
    Ok(())
}

fn load_or_generate_key(
    default_key_path: PathBuf,
    default_cert_path: PathBuf,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
) -> Result<SigningKey> {
    let key_path = key_path.map(Path::to_path_buf);
    let cert_path = cert_path.map(Path::to_path_buf);

    let (key_path, cert_path) = match (key_path, cert_path) {
        (Some(k), Some(c)) => (k, c),
        (None, None) => (default_key_path, default_cert_path),
        _ => bail!("--key and --cert must both be provided"),
    };

    if key_path.exists() && cert_path.exists() {
        info!(
            key = %key_path.display(),
            cert = %cert_path.display(),
            "using existing signing keypair"
        );
    } else {
        info!(
            key = %key_path.display(),
            cert = %cert_path.display(),
            "generating signing keypair"
        );
        let generated = GeneratedKey::generate().context("failed to generate signing key")?;
        generated
            .save(&key_path, &cert_path)
            .context("failed to save signing key")?;
    }

    let key_bytes = std::fs::read(&key_path)
        .with_context(|| format!("failed to read key {}", key_path.display()))?;
    let cert_bytes = std::fs::read(&cert_path)
        .with_context(|| format!("failed to read cert {}", cert_path.display()))?;
    SigningKey::from_pkcs8(&key_bytes, cert_bytes).context("failed to load signing key")
}

fn build_execution_plan(
    bundle: &PatchBundle,
    enabled_patches: &[String],
    disabled_patches: &[String],
    option_args: &[String],
) -> Result<ExecutionPlan> {
    let mut plan = ExecutionPlan::new();

    for patch in enabled_patches {
        plan.select_patch(patch.clone());
    }
    for patch in disabled_patches {
        plan.disable_patch(patch.clone());
    }

    for raw in option_args {
        let (patch_name, option_key, value) = parse_option_arg(raw)?;
        let declaration = find_option_declaration(bundle, &patch_name, &option_key)?;
        let parsed_value = declaration
            .parse_value(&value)
            .with_context(|| format!("failed to parse --option {raw}"))?;

        let mut patch_options = plan
            .options()
            .get(&patch_name)
            .cloned()
            .unwrap_or_else(PatchOptions::new);
        patch_options.set(option_key, parsed_value);
        plan.set_patch_options(patch_name, patch_options);
    }

    Ok(plan)
}

fn parse_option_arg(raw: &str) -> Result<(String, String, String)> {
    let (lhs, value) = raw
        .split_once('=')
        .with_context(|| format!("invalid option '{raw}': expected PATCH.KEY=VALUE"))?;
    let (patch_name, option_key) = lhs
        .split_once('.')
        .with_context(|| format!("invalid option '{raw}': expected PATCH.KEY=VALUE"))?;
    if patch_name.is_empty() || option_key.is_empty() {
        bail!("invalid option '{raw}': patch and key must be non-empty");
    }
    Ok((
        patch_name.to_string(),
        option_key.to_string(),
        value.to_string(),
    ))
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
    println!();
    println!("Paste this into reseam_patcher::bundle::TRUSTED_KEYS:");
    print!("    [");
    for (i, b) in pubkey.iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        print!("0x{b:02x}");
    }
    println!("],");
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
    let seed: [u8; 32] = seed.as_slice().try_into().unwrap();
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

fn find_option_declaration<'a>(
    bundle: &'a PatchBundle,
    patch_name: &str,
    option_key: &str,
) -> Result<&'a OptionDeclaration> {
    let patch = bundle
        .patches
        .iter()
        .find(|patch| patch.name() == patch_name)
        .with_context(|| format!("unknown patch '{patch_name}'"))?;
    patch
        .options()
        .iter()
        .find(|decl| decl.key == option_key)
        .with_context(|| format!("unknown option '{option_key}' for patch '{patch_name}'"))
}
