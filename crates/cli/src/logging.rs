// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Context, Result};
use tracing::Subscriber;
use tracing_subscriber::{EnvFilter, prelude::*};

pub fn init_logging() -> Result<()> {
    let rust_log = match std::env::var("RUST_LOG") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(error) => return Err(error).context("read RUST_LOG"),
    };
    subscriber(rust_log.as_deref())?
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize logging: {error}"))
}

fn subscriber(rust_log: Option<&str>) -> Result<impl Subscriber + Send + Sync> {
    let custom_filter = rust_log.is_some();
    let env_filter = match rust_log {
        Some(value) => EnvFilter::try_new(value).context("invalid RUST_LOG filter")?,
        None => EnvFilter::new(
            "reseam=info,reseam_cli=info,reseam_patcher=info,reseam_apk=info,reseam_sign=info",
        ),
    };
    Ok(tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(custom_filter)
        .with_writer(std::io::stderr)
        .finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;

    #[test]
    fn default_logging_hides_diagnostics_and_rust_log_controls_them() -> Result<()> {
        for (filter, expected) in [
            (None, [true, true, true, false, false]),
            (Some("warn"), [false, true, true, false, false]),
            (Some("debug"), [true, true, true, true, false]),
            (Some("trace"), [true, true, true, true, true]),
            (Some("off"), [false, false, false, false, false]),
        ] {
            tracing::subscriber::with_default(subscriber(filter)?, || {
                assert_eq!(
                    [
                        tracing::enabled!(Level::INFO),
                        tracing::enabled!(Level::WARN),
                        tracing::enabled!(Level::ERROR),
                        tracing::enabled!(Level::DEBUG),
                        tracing::enabled!(Level::TRACE),
                    ],
                    expected
                );
            });
        }
        Ok(())
    }
}
