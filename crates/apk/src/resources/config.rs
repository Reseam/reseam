// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, invalid};

const DENSITY: usize = 14;
const SDK_VERSION: usize = 24;
const UI_MODE: usize = 29;
const MIN_LEN: usize = UI_MODE + 1;

const UI_MODE_NIGHT_NO: u8 = 0x10;
const UI_MODE_NIGHT_YES: u8 = 0x20;

/// A `len`-byte config for `qualifiers`, dash-separated in aapt's order
/// (`xxhdpi`, `night-v26`); an empty string is the default configuration.
pub fn config_for_qualifiers(qualifiers: &str, len: usize) -> Result<Vec<u8>> {
    let len = len.max(4);
    let mut config = vec![0u8; len];
    config[..4].copy_from_slice(&(len as u32).to_le_bytes());
    for qualifier in qualifiers.split('-').filter(|q| !q.is_empty()) {
        if config.len() < MIN_LEN {
            return Err(invalid(
                "resource qualifiers",
                "the package's configs are too short to hold qualifiers",
            ));
        }
        if let Some(density) = density(qualifier) {
            config[DENSITY..DENSITY + 2].copy_from_slice(&density.to_le_bytes());
        } else if let Some(version) = qualifier
            .strip_prefix('v')
            .and_then(|v| v.parse::<u16>().ok())
        {
            config[SDK_VERSION..SDK_VERSION + 2].copy_from_slice(&version.to_le_bytes());
        } else if qualifier == "night" {
            config[UI_MODE] = UI_MODE_NIGHT_YES;
        } else if qualifier == "notnight" {
            config[UI_MODE] = UI_MODE_NIGHT_NO;
        } else {
            return Err(invalid(
                "resource qualifiers",
                format!(
                    "unsupported qualifier '{qualifier}' in '{qualifiers}'; supported are densities, night, notnight and vN"
                ),
            ));
        }
    }
    Ok(config)
}

pub(crate) fn same_config(a: &[u8], b: &[u8]) -> bool {
    let tail = |config: &[u8], i: usize| config.get(i).copied().unwrap_or(0);
    (4..a.len().max(b.len())).all(|i| tail(a, i) == tail(b, i))
}

fn density(qualifier: &str) -> Option<u16> {
    Some(match qualifier {
        "ldpi" => 120,
        "mdpi" => 160,
        "tvdpi" => 213,
        "hdpi" => 240,
        "xhdpi" => 320,
        "xxhdpi" => 480,
        "xxxhdpi" => 640,
        "anydpi" => 0xFFFE,
        "nodpi" => 0xFFFF,
        _ => return qualifier.strip_suffix("dpi")?.parse().ok(),
    })
}
