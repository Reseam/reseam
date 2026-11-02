// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A `ResTable_config` read back into the qualifier string aapt2 writes for
//! it, such as `fr-rCA`, `b+sr+Latn`, `night-v8` or `sw600dp-land-xhdpi`.

use super::DENSITIES;

/// The qualifiers `config` stands for, in aapt2's order and dash-separated;
/// the default configuration is the empty string. Fields past the end of a
/// short config read as unset.
pub fn qualifiers(config: &[u8]) -> String {
    let config = Config(config);
    let mut parts = Vec::new();
    let mut push = |part: Option<String>| parts.extend(part);
    push((config.u16(4) != 0).then(|| format!("mcc{}", config.u16(4))));
    push(match config.u16(6) {
        0 => None,
        0xFFFF => Some("mnc00".to_owned()),
        mnc => Some(format!("mnc{mnc}")),
    });
    push(config.locale());
    push(named(
        config.u8(19),
        &[(1, "neuter"), (2, "feminine"), (3, "masculine")],
    ));
    push(named(
        config.u8(28) & 0xC0,
        &[(0x40, "ldltr"), (0x80, "ldrtl")],
    ));
    push(dp("sw", config.u16(30)));
    push(dp("w", config.u16(32)));
    push(dp("h", config.u16(34)));
    push(named(
        config.u8(28) & 0x0F,
        &[(1, "small"), (2, "normal"), (3, "large"), (4, "xlarge")],
    ));
    push(named(
        config.u8(28) & 0x30,
        &[(0x10, "notlong"), (0x20, "long")],
    ));
    push(named(
        config.u8(48) & 0x03,
        &[(1, "notround"), (2, "round")],
    ));
    push(named(
        config.u8(49) & 0x03,
        &[(1, "nowidecg"), (2, "widecg")],
    ));
    push(named(
        config.u8(49) & 0x0C,
        &[(0x04, "lowdr"), (0x08, "highdr")],
    ));
    push(named(
        config.u8(12),
        &[(1, "port"), (2, "land"), (3, "square")],
    ));
    push(named(
        config.u8(29) & 0x0F,
        &[
            (2, "desk"),
            (3, "car"),
            (4, "television"),
            (5, "appliance"),
            (6, "watch"),
            (7, "vrheadset"),
        ],
    ));
    push(named(
        config.u8(29) & 0x30,
        &[(0x10, "notnight"), (0x20, "night")],
    ));
    push(match config.u16(14) {
        0 => None,
        dpi => Some(
            DENSITIES
                .iter()
                .find(|&&(_, known)| known == dpi)
                .map_or_else(|| format!("{dpi}dpi"), |(name, _)| (*name).to_owned()),
        ),
    });
    push(named(
        config.u8(13),
        &[(1, "notouch"), (2, "stylus"), (3, "finger")],
    ));
    push(named(
        config.u8(18) & 0x03,
        &[(1, "keysexposed"), (2, "keyshidden"), (3, "keyssoft")],
    ));
    push(named(
        config.u8(16),
        &[(1, "nokeys"), (2, "qwerty"), (3, "12key")],
    ));
    push(named(
        config.u8(18) & 0x0C,
        &[(0x04, "navexposed"), (0x08, "navhidden")],
    ));
    push(named(
        config.u8(17),
        &[(1, "nonav"), (2, "dpad"), (3, "trackball"), (4, "wheel")],
    ));
    let (width, height) = (config.u16(20), config.u16(22));
    push((width != 0 || height != 0).then(|| format!("{width}x{height}")));
    push((config.u16(24) != 0).then(|| format!("v{}", config.u16(24))));
    parts.join("-")
}

fn named(value: u8, names: &[(u8, &str)]) -> Option<String> {
    names
        .iter()
        .find(|&&(code, _)| code == value)
        .map(|(_, name)| (*name).to_owned())
}

fn dp(prefix: &str, value: u16) -> Option<String> {
    (value != 0).then(|| format!("{prefix}{value}dp"))
}

struct Config<'a>(&'a [u8]);

impl Config<'_> {
    fn u8(&self, offset: usize) -> u8 {
        self.0.get(offset).copied().unwrap_or(0)
    }

    fn u16(&self, offset: usize) -> u16 {
        u16::from_le_bytes([self.u8(offset), self.u8(offset + 1)])
    }

    fn text(&self, offset: usize, len: usize) -> String {
        (offset..offset + len)
            .map(|i| self.u8(i))
            .take_while(|&byte| byte != 0)
            .map(char::from)
            .collect()
    }

    /// `lang` or `lang-rREGION`, three-letter codes included, or the BCP 47
    /// form `b+lang+Script+REGION+variant` when a script, variant or numbering
    /// system needs it.
    fn locale(&self) -> Option<String> {
        let language = unpack(self.u8(8), self.u8(9), b'a');
        let region = unpack(self.u8(10), self.u8(11), b'0');
        // A script the runtime derived from the language is not a qualifier.
        let script = if self.u8(52) == 0 {
            self.text(36, 4)
        } else {
            String::new()
        };
        let variant = self.text(40, 8);
        let numbering = self.text(53, 8);
        if language.is_empty() && region.is_empty() {
            return None;
        }
        if script.is_empty() && variant.is_empty() && numbering.is_empty() {
            return Some(if region.is_empty() {
                language
            } else {
                format!("{language}-r{region}")
            });
        }
        let mut locale = format!("b+{language}");
        for part in [script, region, variant] {
            if !part.is_empty() {
                locale.push('+');
                locale.push_str(&part);
            }
        }
        if !numbering.is_empty() {
            locale.push_str("+u+nu+");
            locale.push_str(&numbering);
        }
        Some(locale)
    }
}

/// A language or region code from its two bytes: two letters as they are, or
/// three letters packed into five bits each when the high bit is set.
fn unpack(first: u8, second: u8, base: u8) -> String {
    if first & 0x80 == 0 {
        return [first, second]
            .into_iter()
            .take_while(|&byte| byte != 0)
            .map(char::from)
            .collect();
    }
    [
        second & 0x1f,
        ((second & 0xe0) >> 5) | ((first & 0x03) << 3),
        (first & 0x7c) >> 2,
    ]
    .into_iter()
    .map(|letter| char::from(base + letter))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::super::config_for_qualifiers;
    use super::qualifiers;

    #[test]
    fn written_configs_read_back_as_their_qualifiers() {
        for written in ["", "xxhdpi", "night", "notnight-v26", "nodpi", "560dpi-v31"] {
            let config = config_for_qualifiers(written, 64).expect("supported qualifiers");
            assert_eq!(qualifiers(&config), written);
        }
    }
}
