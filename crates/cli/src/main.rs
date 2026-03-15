use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "stitch", about = "High-performance APK patching engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Patch an APK with a patch bundle
    Patch {
        /// Path to the input APK
        apk: String,
        /// Path to the patch bundle directory
        #[arg(short, long)]
        patches: String,
        /// Output path (default: <input>-patched.apk)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// List patches in a bundle
    List {
        /// Path to the patch bundle directory
        patches: String,
    },
    /// Show info about an APK (DEX count, package name, version)
    Info {
        /// Path to the APK
        apk: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Patch { apk, patches, output } => {
            let out = output.unwrap_or_else(|| apk.replace(".apk", "-patched.apk"));
            eprintln!("[stitch] Input:   {apk}");
            eprintln!("[stitch] Patches: {patches}");
            eprintln!("[stitch] Output:  {out}");
            eprintln!("[stitch] Patching not yet implemented");
            Ok(())
        }
        Commands::List { patches } => {
            eprintln!("[stitch] Bundle: {patches}");
            eprintln!("[stitch] Listing not yet implemented");
            Ok(())
        }
        Commands::Info { apk } => {
            eprintln!("[stitch] APK: {apk}");
            eprintln!("[stitch] Info not yet implemented");
            Ok(())
        }
    }
}
