// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use dprint_plugin_typescript::configuration::{ConfigurationBuilder, QuoteProps, QuoteStyle};
use dprint_plugin_typescript::{FormatTextOptions, format_text};
use reseam_model as model;
use ts_rs::{Config, TS};

type Declaration = fn(&Config) -> String;

/// Every model type the browser package exchanges with the engine.
const DECLARATIONS: [Declaration; 37] = [
    declaration::<model::InspectRequest>,
    declaration::<model::InspectResponse>,
    declaration::<model::ApkMetadata>,
    declaration::<model::ContainerFormat>,
    declaration::<model::BundleMetadata>,
    declaration::<model::PatchMetadata>,
    declaration::<model::PatchSpec>,
    declaration::<model::PatchRef>,
    declaration::<model::PatchPreset>,
    declaration::<model::Compatibility>,
    declaration::<model::CompatiblePackage>,
    declaration::<model::Incompatibility>,
    declaration::<model::OptionDeclaration>,
    declaration::<model::OptionType>,
    declaration::<model::OptionValue>,
    declaration::<model::PatchRequest>,
    declaration::<model::Trust>,
    declaration::<model::PatchSelection>,
    declaration::<model::PatchOutput>,
    declaration::<model::SigningKeyFiles>,
    declaration::<model::InstallMethod>,
    declaration::<model::PatchOutcome>,
    declaration::<model::PatchArtifact>,
    declaration::<model::PatchResult>,
    declaration::<model::PatchStatus>,
    declaration::<model::LogEntry>,
    declaration::<model::LogLevel>,
    declaration::<model::RunEvent>,
    declaration::<model::PatchMetrics>,
    declaration::<model::PatchPhase>,
    declaration::<model::PatchPhaseMetrics>,
    declaration::<model::ApplyDiagnostics>,
    declaration::<model::MemoryBreakdown>,
    declaration::<model::MaterializationStats>,
    declaration::<model::JvmHeapStats>,
    declaration::<model::Problem>,
    declaration::<model::SdkError>,
];

fn declaration<T: TS>(config: &Config) -> String {
    format!("export {}\n", T::decl(config))
}

fn path() -> PathBuf {
    crate::paths::workspace_root().join("browser/src/models.ts")
}

/// The browser package's `models.ts`, generated from the Rust model.
fn source() -> Result<String> {
    // Engine JSON numbers are JavaScript numbers; only patch option longs can exceed them.
    let config = Config::new().with_large_int("number");
    let text = DECLARATIONS.iter().fold(
        String::from(
            "// Generated from crates/model by `cargo xtask regen browser`. Do not edit.\n",
        ),
        |mut source, declaration| {
            source.push_str(&declaration(&config));
            source
        },
    );
    let style = ConfigurationBuilder::new()
        .deno()
        .line_width(120)
        .quote_style(QuoteStyle::PreferSingle)
        .quote_props(QuoteProps::AsNeeded)
        .build();
    let path = path();
    let formatted = format_text(FormatTextOptions {
        path: &path,
        extension: None,
        text: text.clone(),
        config: &style,
        external_formatter: None,
    })?;
    Ok(formatted.unwrap_or(text))
}

pub fn regen() -> Result<()> {
    Ok(fs::write(path(), source()?)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn browser_models_match_the_rust_model() {
        assert!(
            std::fs::read_to_string(super::path()).unwrap() == super::source().unwrap(),
            "browser/src/models.ts is stale; run `cargo xtask regen browser`"
        );
    }
}
