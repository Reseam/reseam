// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod locale;
mod qualifiers;

use std::iter::Peekable;

pub use qualifiers::qualifiers;

use crate::error::{Result, invalid};
use locale::{LANGUAGE_BASE, REGION_BASE, pack};

const LANGUAGE: usize = 8;
const REGION: usize = 10;
const DENSITY: usize = 14;
const SDK_VERSION: usize = 24;
const SCREEN_LAYOUT: usize = 28;
const UI_MODE: usize = 29;
const MIN_LEN: usize = UI_MODE + 1;

const SCREEN_SIZE_MASK: u8 = 0x0f;
const SCREEN_SIZES: [(u8, &str); 4] = [(1, "small"), (2, "normal"), (3, "large"), (4, "xlarge")];

const NIGHT_MASK: u8 = 0x30;
const NIGHT_MODES: [(u8, &str); 2] = [(0x10, "notnight"), (0x20, "night")];

/// Density qualifiers aapt names, and the dots per inch each stands for.
const DENSITIES: [(&str, u16); 9] = [
    ("ldpi", 120),
    ("mdpi", 160),
    ("tvdpi", 213),
    ("hdpi", 240),
    ("xhdpi", 320),
    ("xxhdpi", 480),
    ("xxxhdpi", 640),
    ("anydpi", 0xFFFE),
    ("nodpi", 0xFFFF),
];

/// A `len`-byte config for `qualifiers`, dash-separated in aapt's order: a language and its
/// region, a screen size, `night` or `notnight`, a density, then `vN`, as in `pt-rBR-xlarge`,
/// `night-xxhdpi` or `anydpi-v26`. An empty string is the default configuration.
pub fn config_for_qualifiers(qualifiers: &str, len: usize) -> Result<Vec<u8>> {
    let len = len.max(4);
    let mut config = vec![0u8; len];
    config[..4].copy_from_slice(&(len as u32).to_le_bytes());
    let mut parts = qualifiers
        .split('-')
        .filter(|part| !part.is_empty())
        .peekable();
    if parts.peek().is_none() {
        return Ok(config);
    }
    if config.len() < MIN_LEN {
        return Err(invalid(
            "resource qualifiers",
            "the package's configs are too short to hold qualifiers",
        ));
    }
    if let Some(language) = take(&mut parts, language) {
        config[LANGUAGE..LANGUAGE + 2].copy_from_slice(&language);
        if let Some(region) = take(&mut parts, region) {
            config[REGION..REGION + 2].copy_from_slice(&region);
        }
    }
    if let Some(size) = take(&mut parts, |part| code(&SCREEN_SIZES, part)) {
        config[SCREEN_LAYOUT] = (config[SCREEN_LAYOUT] & !SCREEN_SIZE_MASK) | size;
    }
    if let Some(night) = take(&mut parts, |part| code(&NIGHT_MODES, part)) {
        config[UI_MODE] = (config[UI_MODE] & !NIGHT_MASK) | night;
    }
    if let Some(density) = take(&mut parts, density) {
        config[DENSITY..DENSITY + 2].copy_from_slice(&density.to_le_bytes());
    }
    if let Some(version) = take(&mut parts, |part| {
        part.strip_prefix('v')?.parse::<u16>().ok()
    }) {
        config[SDK_VERSION..SDK_VERSION + 2].copy_from_slice(&version.to_le_bytes());
    }
    match parts.next() {
        None => Ok(config),
        Some(part) => Err(invalid(
            "resource qualifiers",
            format!(
                "unsupported or out-of-order qualifier '{part}' in '{qualifiers}'; supported are a language and region, a screen size, night or notnight, a density and vN, in that order"
            ),
        )),
    }
}

fn take<'a, T>(
    parts: &mut Peekable<impl Iterator<Item = &'a str>>,
    parse: impl Fn(&str) -> Option<T>,
) -> Option<T> {
    let value = parse(parts.peek()?)?;
    parts.next();
    Some(value)
}

fn code(names: &[(u8, &str)], part: &str) -> Option<u8> {
    names
        .iter()
        .find(|&&(_, name)| name == part)
        .map(|&(code, _)| code)
}

pub(crate) fn same_config(a: &[u8], b: &[u8]) -> bool {
    let tail = |config: &[u8], i: usize| config.get(i).copied().unwrap_or(0);
    (4..a.len().max(b.len())).all(|i| tail(a, i) == tail(b, i))
}

/// A language as aapt reads one: two or three letters, except `car`, which names a UI mode.
fn language(part: &str) -> Option<[u8; 2]> {
    (matches!(part.len(), 2 | 3) && part.bytes().all(|b| b.is_ascii_lowercase()) && part != "car")
        .then(|| pack(part.as_bytes(), LANGUAGE_BASE))
}

fn region(part: &str) -> Option<[u8; 2]> {
    let code = part.strip_prefix('r')?.as_bytes();
    let letters = code.len() == 2 && code.iter().all(u8::is_ascii_uppercase);
    let digits = code.len() == 3 && code.iter().all(u8::is_ascii_digit);
    (letters || digits).then(|| pack(code, REGION_BASE))
}

fn density(qualifier: &str) -> Option<u16> {
    DENSITIES
        .iter()
        .find(|(name, _)| *name == qualifier)
        .map(|&(_, dpi)| dpi)
        .or_else(|| qualifier.strip_suffix("dpi")?.parse().ok())
}
