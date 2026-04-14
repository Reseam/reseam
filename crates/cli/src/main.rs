use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use stitch_apk::ApkFile;
use stitch_patcher::bundle::PatchBundle;
use stitch_patcher::context::PatchContext;
use stitch_patcher::engine::{self, ExecutionPlan, PatchStatus};
use stitch_patcher::options::{OptionDeclaration, PatchOptions};
use stitch_sign::{GeneratedKey, SigningKey};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

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
        #[arg(short, long, requires = "cert")]
        key: Option<PathBuf>,
        #[arg(short, long, requires = "key")]
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
    List {
        bundle: PathBuf,
    },
    Info {
        apk: PathBuf,
    },
    Bundle {
        #[command(subcommand)]
        command: BundleCommands,
    },
}

#[derive(Subcommand)]
enum BundleCommands {
    Keygen {
        #[arg(short, long)]
        out: PathBuf,
    },
    Pack {
        dir: PathBuf,
        #[arg(short, long)]
        key: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    init_logging()?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Patch {
            apk,
            bundle,
            output,
            key,
            cert,
            enable,
            disable,
            option,
            dry_run,
        } => cmd_patch(
            &apk,
            &bundle,
            output.as_deref(),
            key.as_deref(),
            cert.as_deref(),
            &enable,
            &disable,
            &option,
            dry_run,
        ),
        Commands::List { bundle } => cmd_list(&bundle),
        Commands::Info { apk } => cmd_info(&apk),
        Commands::Bundle { command } => match command {
            BundleCommands::Keygen { out } => cmd_bundle_keygen(&out),
            BundleCommands::Pack { dir, key, out } => cmd_bundle_pack(&dir, &key, &out),
        },
    }
}

fn init_logging() -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "stitch=info,stitch_cli=info,stitch_patcher=info,stitch_apk=info,stitch_sign=info",
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
    bundle_path: &Path,
    output: Option<&Path>,
    key_path: Option<&Path>,
    cert_path: Option<&Path>,
    enabled_patches: &[String],
    disabled_patches: &[String],
    option_args: &[String],
    dry_run: bool,
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

    info!(apk_path = %apk_path.display(), "opening APK");
    let mut apk = ApkFile::open(apk_path).context("failed to open APK")?;

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

    let mut ctx = PatchContext::new(&mut apk);

    if !patch_bundle.extension_dex.is_empty() {
        let count = ctx
            .merge_extension_dex(&patch_bundle.extension_dex)
            .context("failed to merge extension DEX")?;
        info!(count, "merged bundle extension DEX files");
    }

    let results = engine::apply_patches_with_plan(&mut ctx, &patch_bundle.patches, &plan)
        .context("patch application failed")?;
    drop(ctx);

    for result in &results {
        match &result.status {
            PatchStatus::Applied => info!(patch = %result.name, "patch applied"),
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
        "patch run completed"
    );
    if failed_count > 0 {
        bail!("{failed_count} patch(es) failed");
    }

    if dry_run {
        info!("dry run enabled; not writing output");
        return Ok(());
    }

    info!(output_path = %output_path.display(), "writing patched APK");
    let tmp_dir = tempfile::tempdir().context("failed to create temp directory")?;
    apk.write_to(tmp_dir.path())
        .context("failed to write patched APK")?;
    info!("patched APK written to temp directory");

    let tmp_apk_path = find_apk_in_dir(tmp_dir.path())?;
    let unsigned_bytes =
        std::fs::read(&tmp_apk_path).context("failed to read patched APK bytes")?;
    info!(
        unsigned_size = unsigned_bytes.len(),
        "loaded patched APK bytes"
    );

    let signing_key = load_or_generate_key(key_path, cert_path)?;

    info!("signing APK with Signature Scheme v2");
    let signed_bytes =
        stitch_sign::v2::sign(&unsigned_bytes, &signing_key).context("v2 signing failed")?;

    std::fs::write(&output_path, &signed_bytes)
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    info!(
        output_path = %output_path.display(),
        size_mb = signed_bytes.len() as f64 / (1024.0 * 1024.0),
        "patched APK written"
    );

    Ok(())
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
        let p: &dyn stitch_patcher::patch::Patch = patch.as_ref();
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

fn find_apk_in_dir(dir: &Path) -> Result<PathBuf> {
    for entry in std::fs::read_dir(dir).context("failed to read temp directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "apk") {
            return Ok(path);
        }
    }
    bail!("no APK file found in output directory")
}

const DEFAULT_KEY: &str = "stitch.pk8";
const DEFAULT_CERT: &str = "stitch.der";

fn load_or_generate_key(key_path: Option<&Path>, cert_path: Option<&Path>) -> Result<SigningKey> {
    let key_path = key_path.map(Path::to_path_buf);
    let cert_path = cert_path.map(Path::to_path_buf);

    let (key_path, cert_path) = match (key_path, cert_path) {
        (Some(k), Some(c)) => (k, c),
        (None, None) if Path::new(DEFAULT_KEY).exists() && Path::new(DEFAULT_CERT).exists() => {
            info!(
                key = DEFAULT_KEY,
                cert = DEFAULT_CERT,
                "using existing signing keypair"
            );
            (PathBuf::from(DEFAULT_KEY), PathBuf::from(DEFAULT_CERT))
        }
        (None, None) => {
            info!(
                key = DEFAULT_KEY,
                cert = DEFAULT_CERT,
                "generating signing keypair"
            );
            let generated = GeneratedKey::generate().context("failed to generate signing key")?;
            generated
                .save(Path::new(DEFAULT_KEY), Path::new(DEFAULT_CERT))
                .context("failed to save signing key")?;
            return Ok(generated.signing_key);
        }
        _ => bail!("--key and --cert must both be provided"),
    };

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
    println!("Paste this into stitch_patcher::bundle::TRUSTED_KEYS:");
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
    if partial.bundle.format_version != stitch_patcher::bundle::BUNDLE_FORMAT_VERSION {
        bail!(
            "unsupported format_version {}; CLI supports {}",
            partial.bundle.format_version,
            stitch_patcher::bundle::BUNDLE_FORMAT_VERSION
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
        manifest.push_str(&format!("author = {}\n", toml_string(&partial.bundle.author)));
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

    let seed = std::fs::read(key_path)
        .with_context(|| format!("read key {}", key_path.display()))?;
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
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("mimetype", stored)?;
    zip.write_all(stitch_patcher::bundle::BUNDLE_MIMETYPE.as_bytes())?;
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
