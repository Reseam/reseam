use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use stitch_apk::ApkFile;
use stitch_patcher::bundle::PatchBundle;
use stitch_patcher::context::PatchContext;
use stitch_patcher::engine::{self, PatchStatus};
use stitch_sign::{GeneratedKey, SigningKey};

#[derive(Parser)]
#[command(name = "stitch", about = "High-performance APK patching engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Patch {
        apk: PathBuf,
        #[arg(short, long)]
        bundle: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        key: Option<PathBuf>,
        #[arg(short, long)]
        cert: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        verbose: bool,
    },
    List {
        bundle: PathBuf,
    },
    Info {
        apk: PathBuf,
    },
}

fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGSEGV, sigsegv_handler as *const () as libc::sighandler_t);
        libc::signal(libc::SIGBUS, sigsegv_handler as *const () as libc::sighandler_t);
        libc::signal(libc::SIGABRT, sigsegv_handler as *const () as libc::sighandler_t);
    }
}

extern "C" fn sigsegv_handler(sig: libc::c_int) {
    let msg = match sig {
        libc::SIGSEGV => "[stitch] FATAL: Segmentation fault (SIGSEGV) — likely ABI mismatch in native patch. Rebuild patches with: cargo build --release\n",
        libc::SIGBUS => "[stitch] FATAL: Bus error (SIGBUS)\n",
        libc::SIGABRT => "[stitch] FATAL: Abort (SIGABRT)\n",
        _ => "[stitch] FATAL: Unexpected signal\n",
    };
    unsafe {
        libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
        libc::_exit(139);
    }
}

fn main() -> Result<()> {
    install_signal_handlers();
    let cli = Cli::parse();

    match cli.command {
        Commands::Patch {
            apk,
            bundle,
            output,
            key,
            cert,
            dry_run,
            verbose,
        } => cmd_patch(&apk, &bundle, output.as_deref(), key.as_deref(), cert.as_deref(), dry_run, verbose),
        Commands::List { bundle } => cmd_list(&bundle),
        Commands::Info { apk } => cmd_info(&apk),
    }
}

fn cmd_patch(
    apk_path: &Path,
    bundle_path: &Path,
    output: Option<&Path>,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let output_path = match output {
        Some(p) => p.to_path_buf(),
        None => {
            let stem = apk_path
                .file_stem()
                .context("invalid APK path")?
                .to_string_lossy();
            apk_path.with_file_name(format!("{stem}-patched.apk"))
        }
    };

    eprintln!("[stitch] opening {}", apk_path.display());
    let mut apk = ApkFile::open(apk_path).context("failed to open APK")?;

    if let Some(pkg) = apk.package_name() {
        eprintln!("[stitch] package: {pkg}");
    }
    if let Some(ver) = apk.version_name() {
        eprintln!("[stitch] version: {ver}");
    }
    eprintln!("[stitch] dex files: {}", apk.dex().len());

    eprintln!("[stitch] loading bundle {}", bundle_path.display());
    let patch_bundle = PatchBundle::load(bundle_path).context("failed to load patch bundle")?;
    eprintln!(
        "[stitch] bundle '{}' ({} patches)",
        patch_bundle.name,
        patch_bundle.patches.len()
    );

    let mut ctx = PatchContext::new(&mut apk);

    if !patch_bundle.extension_dex.is_empty() {
        let count = ctx
            .merge_extension_dex(&patch_bundle.extension_dex)
            .context("failed to merge extension DEX")?;
        eprintln!("[stitch] merged {count} extension DEX files");
    }

    let results = engine::apply_patches(&mut ctx, &patch_bundle.patches)
        .context("patch application failed")?;
    drop(ctx);

    for result in &results {
        if verbose {
            for log in &result.logs {
                eprintln!("{log}");
            }
        }
        match &result.status {
            PatchStatus::Applied => eprintln!("[stitch] applied: {}", result.name),
            PatchStatus::Skipped { reason } => {
                eprintln!("[stitch] skipped: {} ({reason})", result.name)
            }
            PatchStatus::Failed { reason } => {
                eprintln!("[stitch] FAILED: {} ({reason})", result.name)
            }
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
    if failed_count > 0 {
        eprintln!("[stitch] WARNING: {failed_count} patch(es) failed");
    }
    eprintln!("[stitch] {applied_count}/{} patches applied", results.len());

    if dry_run {
        eprintln!("[stitch] dry run — not writing output");
        return Ok(());
    }

    eprintln!("[stitch] writing patched APK...");
    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    apk.write_to(tmp_dir.path())
        .context("failed to write patched APK")?;
    eprintln!("[stitch] APK written, reading back...");

    let tmp_apk_path = find_apk_in_dir(tmp_dir.path())?;
    let unsigned_bytes =
        std::fs::read(&tmp_apk_path).context("failed to read patched APK bytes")?;
    eprintln!("[stitch] read {} bytes, loading signing key...", unsigned_bytes.len());

    let signing_key = load_or_generate_key(key_path, cert_path)?;

    eprintln!("[stitch] signing with APK Signature Scheme v2");
    let signed_bytes =
        stitch_sign::v2::sign(&unsigned_bytes, &signing_key).context("v2 signing failed")?;

    std::fs::write(&output_path, &signed_bytes)
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    eprintln!(
        "[stitch] done: {} ({:.1} MB)",
        output_path.display(),
        signed_bytes.len() as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}

fn cmd_list(bundle_path: &Path) -> Result<()> {
    let bundle = PatchBundle::load(bundle_path).context("failed to load patch bundle")?;
    eprintln!("[stitch] bundle: {}", bundle.name);
    if !bundle.author.is_empty() {
        eprintln!("[stitch] author: {}", bundle.author);
    }
    if !bundle.description.is_empty() {
        eprintln!("[stitch] description: {}", bundle.description);
    }
    eprintln!();
    for (i, patch) in bundle.patches.iter().enumerate() {
        let p: &dyn stitch_patcher::patch::Patch = patch.as_ref();
        let enabled = if p.enabled_by_default() { "on" } else { "off" };
        eprintln!(
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
            eprintln!("       packages: {}", formatted.join(", "));
        }
    }

    if !bundle.extension_dex.is_empty() {
        eprintln!();
        eprintln!("[stitch] extension DEX:");
        for dex_path in &bundle.extension_dex {
            eprintln!("  - {}", dex_path.display());
        }
    }

    Ok(())
}

fn cmd_info(apk_path: &Path) -> Result<()> {
    let apk = ApkFile::open(apk_path).context("failed to open APK")?;

    eprintln!("APK: {}", apk_path.display());
    if let Some(pkg) = apk.package_name() {
        eprintln!("  package:    {pkg}");
    }
    if let Some(ver) = apk.version_name() {
        eprintln!("  version:    {ver}");
    }
    if let Some(code) = apk.version_code() {
        eprintln!("  versionCode: {code}");
    }
    eprintln!("  dex files:  {}", apk.dex().len());
    eprintln!("  components: {}", apk.component_count());
    if apk.is_split() {
        eprintln!("  splits:     {}", apk.split_names().join(", "));
    }

    let total_classes: usize = apk.dex().iter().map(|d| d.classes.len()).sum();
    let total_methods: usize = apk.dex().iter().map(|d| d.methods.len()).sum();
    eprintln!("  classes:    {total_classes}");
    eprintln!("  methods:    {total_methods}");

    Ok(())
}

fn find_apk_in_dir(dir: &Path) -> Result<PathBuf> {
    for entry in std::fs::read_dir(dir).context("failed to read temp directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "apk") {
            return Ok(path);
        }
    }
    bail!("no APK file found in output directory")
}

const DEFAULT_KEY: &str = "stitch.pk8";
const DEFAULT_CERT: &str = "stitch.der";

fn load_or_generate_key(
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
) -> Result<SigningKey> {
    let key_path = key_path.map(Path::to_path_buf);
    let cert_path = cert_path.map(Path::to_path_buf);

    let (key_path, cert_path) = match (key_path, cert_path) {
        (Some(k), Some(c)) => (k, c),
        (None, None) if Path::new(DEFAULT_KEY).exists() && Path::new(DEFAULT_CERT).exists() => {
            eprintln!("[stitch] using existing key {DEFAULT_KEY} + {DEFAULT_CERT}");
            (PathBuf::from(DEFAULT_KEY), PathBuf::from(DEFAULT_CERT))
        }
        (None, None) => {
            eprintln!("[stitch] generating ECDSA P-256 key, saving to {DEFAULT_KEY} + {DEFAULT_CERT}");
            let generated = GeneratedKey::generate().context("failed to generate signing key")?;
            generated
                .save(Path::new(DEFAULT_KEY), Path::new(DEFAULT_CERT))
                .context("failed to save signing key")?;
            return Ok(generated.signing_key);
        }
        _ => bail!("--key and --cert must both be provided"),
    };

    let key_bytes =
        std::fs::read(&key_path).with_context(|| format!("failed to read key {}", key_path.display()))?;
    let cert_bytes =
        std::fs::read(&cert_path).with_context(|| format!("failed to read cert {}", cert_path.display()))?;
    SigningKey::from_pkcs8(&key_bytes, cert_bytes).context("failed to load signing key")
}
