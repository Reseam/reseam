// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;
use std::error::Error;
use std::path::PathBuf;

/// JDK overrides in cc's per-target order, followed by the host default.
pub fn variables(target: &str) -> [String; 3] {
    [
        format!("JAVA_HOME_{target}"),
        format!("JAVA_HOME_{}", target.replace(['-', '.'], "_")),
        "JAVA_HOME".into(),
    ]
}

/// Selects a JDK without executing it, so a target JDK can live on another OS.
/// An explicit override is authoritative; invalid paths never fall back.
pub fn home(target: &str) -> Result<PathBuf, Box<dyn Error>> {
    let (variable, value) = variables(target)
        .into_iter()
        .find_map(|variable| env::var_os(&variable).map(|value| (variable, value)))
        .ok_or_else(|| {
            format!(
                "set JAVA_HOME or JAVA_HOME_{} to a JDK",
                target.replace('-', "_")
            )
        })?;
    let home = PathBuf::from(value);
    let platform = platform(target);
    for header in ["jni.h", &format!("{platform}/jni_md.h")] {
        let path = home.join("include").join(header);
        let metadata = std::fs::metadata(&path).map_err(|error| {
            format!(
                "{variable}: cannot read JNI header {}: {error}",
                path.display()
            )
        })?;
        if !metadata.is_file() {
            return Err(format!("{variable}: JNI header {} is not a file", path.display()).into());
        }
    }
    Ok(home)
}

/// The platform directory inside a desktop JDK's include directory.
pub fn platform(target: &str) -> &'static str {
    if target.contains("windows") {
        "win32"
    } else if target.contains("apple") {
        "darwin"
    } else {
        "linux"
    }
}
