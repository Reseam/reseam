// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::is_entry_name;
use crate::error::{Result, invalid};

/// Writes `apks`, each a name and the file holding it, into a new split
/// archive at `path`: the APKs at the root of a ZIP, as [`open_container`]
/// reads an APKS file. APKs are compressed already, so they are stored.
///
/// [`open_container`]: super::open_container
pub fn write_split_archive(apks: &[(&str, &Path)], path: &Path) -> Result<()> {
    let mut zip = ZipWriter::new(BufWriter::new(File::create_new(path)?));
    for &(name, apk) in apks {
        let named_apk = Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("apk"));
        if !named_apk || !is_entry_name(name) {
            return Err(invalid(
                "container",
                format!("split archive entries are APK file names: {name}"),
            ));
        }
        let mut source = File::open(apk)?;
        let large = source.metadata()?.len() >= u64::from(u32::MAX);
        zip.start_file(
            name,
            SimpleFileOptions::default()
                .compression_method(CompressionMethod::Stored)
                .large_file(large),
        )?;
        io::copy(&mut source, &mut zip)?;
    }
    zip.finish()?.flush()?;
    Ok(())
}
