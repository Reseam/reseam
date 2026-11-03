// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use reseam_model::{InstalledApp, InstalledSplit};

use crate::error::{HostError, Result};

/// The installed app as the output of [`query_script`](super::query_script)
/// describes it, or none when it is not installed. Android names an installed
/// app's files `base.apk` and `split_<name>.apk`.
pub fn installed_app(output: &str) -> Result<Option<InstalledApp>> {
    if output.trim().is_empty() {
        return Ok(None);
    }
    let unexpected = |line: &str| HostError::InstalledAppQuery(format!("the line {line:?}"));
    let mut version_code = None;
    let mut version_name = String::new();
    let mut base_file = None;
    let mut splits = Vec::new();
    let mut dex_metadata_file = None;
    for line in output.lines() {
        let (key, value) = line.split_once('=').ok_or_else(|| unexpected(line))?;
        match key {
            "version_code" => version_code = Some(value.parse().map_err(|_| unexpected(line))?),
            "version_name" => value.clone_into(&mut version_name),
            "file"
                if Path::new(value)
                    .extension()
                    .is_some_and(|extension| extension == "dm") =>
            {
                dex_metadata_file = Some(value.to_owned());
            }
            "file" => match value
                .strip_prefix("split_")
                .and_then(|name| name.strip_suffix(".apk"))
            {
                Some(name) => splits.push(InstalledSplit {
                    name: name.to_owned(),
                    file: value.to_owned(),
                }),
                None => base_file = Some(value.to_owned()),
            },
            _ => return Err(unexpected(line)),
        }
    }
    Ok(Some(InstalledApp {
        version_code: version_code
            .ok_or_else(|| HostError::InstalledAppQuery("no version code".to_owned()))?,
        version_name,
        base_file: base_file
            .ok_or_else(|| HostError::InstalledAppQuery("no base APK".to_owned()))?,
        splits,
        dex_metadata_file,
    }))
}
