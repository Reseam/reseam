// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;

use reseam_model::analysis::{Runtime, RuntimeKind};

/// The file a React Native app's JavaScript ships in.
pub(super) const JS_BUNDLE: &str = "assets/index.android.bundle";

/// The first eight bytes of a Hermes bytecode file, little endian.
const HERMES_MAGIC: u64 = 0x1F19_03C1_03BC_1FC6;

/// Runtimes other than the DEX that hold app logic, from the entry names of
/// every component. `bundle_header` is the first 12 bytes of the React Native
/// bundle, when there is one.
pub(super) fn detect(entries: &[String], bundle_header: Option<&[u8]>) -> Vec<Runtime> {
    let mut runtimes = Vec::new();
    let mut found = |kind, evidence: BTreeSet<&str>| {
        if !evidence.is_empty() {
            runtimes.push(Runtime {
                kind,
                evidence: evidence.into_iter().map(str::to_owned).collect(),
            });
        }
    };
    let libraries = |names: &[&str]| -> BTreeSet<&str> {
        entries
            .iter()
            .filter_map(|entry| entry.strip_prefix("lib/")?.split_once('/'))
            .map(|(_, file)| file)
            .filter(|file| names.contains(file))
            .collect()
    };
    found(
        RuntimeKind::Flutter,
        libraries(&["libapp.so", "libflutter.so"]),
    );
    if let Some(header) = bundle_header {
        let kind = match header {
            [magic @ .., v0, v1, v2, v3]
                if magic.len() == 8
                    && u64::from_le_bytes(magic.try_into().expect("eight bytes"))
                        == HERMES_MAGIC =>
            {
                RuntimeKind::Hermes {
                    bytecode_version: u32::from_le_bytes([*v0, *v1, *v2, *v3]),
                }
            }
            _ => RuntimeKind::JavaScript,
        };
        found(kind, BTreeSet::from([JS_BUNDLE]));
    }
    let mut unity = libraries(&["libil2cpp.so", "libunity.so"]);
    unity.extend(
        entries
            .iter()
            .map(String::as_str)
            .filter(|entry| entry.starts_with("assets/bin/Data/Managed/")),
    );
    found(RuntimeKind::Unity, unity);
    let mut dotnet = libraries(&["libmonodroid.so", "libmonosgen-2.0.so"]);
    dotnet.extend(entries.iter().map(String::as_str).filter(|entry| {
        entry.starts_with("assemblies/") && has_extension(entry, "dll")
            || entry.starts_with("lib/") && entry.contains("libassemblies.")
    }));
    found(RuntimeKind::DotNet, dotnet);
    runtimes
}

fn has_extension(entry: &str, extension: &str) -> bool {
    std::path::Path::new(entry)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}
