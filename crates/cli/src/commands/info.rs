// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use reseam_library::inspect_apk as inspect_apk_with_library;

use crate::app::InfoCommand;

pub fn run_info(command: &InfoCommand) -> Result<()> {
    let apk = inspect_apk_with_library(&command.apk, &[])?;

    println!("APK: {}", command.apk.display());
    if let Some(package) = apk.package_name {
        println!("  package:    {package}");
    }
    if let Some(version) = apk.version_name {
        println!("  version:    {version}");
    }
    if let Some(code) = apk.version_code {
        println!("  versionCode: {code}");
    }
    println!("  dex files:  {}", apk.dex_files);
    println!("  components: {}", apk.component_count);
    if !apk.split_names.is_empty() {
        println!("  splits:     {}", apk.split_names.join(", "));
    }
    println!("  classes:    {}", apk.class_count);
    println!("  methods:    {}", apk.method_count);

    Ok(())
}
