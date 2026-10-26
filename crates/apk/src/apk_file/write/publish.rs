// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use tracing::instrument;

use super::ApkWriteOptions;
use crate::apk_file::ApkFile;
use crate::error::{Result, invalid};

impl ApkFile {
    /// Writes complete component APKs under their input file names. Output is
    /// staged in temporary files before publication. Duplicate output names and
    /// paths pointing to the input are errors. Dirty state is retained, so the
    /// session can be written repeatedly.
    /// Edited v041 containers retain their physical entry and logical member
    /// order, including any pool-sized parts required by overflow.
    #[instrument(level = "info", skip_all)]
    pub fn write_to(
        &self,
        output_dir: impl AsRef<Path>,
        options: ApkWriteOptions,
    ) -> Result<Vec<PathBuf>> {
        let output_dir = output_dir.as_ref();
        std::fs::create_dir_all(output_dir)?;
        let names = self.output_names()?;
        let paths: Vec<_> = names.iter().map(|name| output_dir.join(name)).collect();
        for path in &paths {
            if path.exists() {
                let destination = reseam_storage::canonicalize(path)?;
                for component in &self.components {
                    if destination == reseam_storage::canonicalize(component.path())? {
                        return Err(invalid(
                            "apk output",
                            format!("{} is an input APK", path.display()),
                        ));
                    }
                }
            }
        }
        let staging = paths
            .iter()
            .map(|_| tempfile::NamedTempFile::new_in(output_dir))
            .collect::<io::Result<Vec<_>>>()?;
        self.write_components(
            options,
            staging
                .iter()
                .map(tempfile::NamedTempFile::reopen)
                .collect::<io::Result<_>>()?,
        )?;
        for (file, path) in staging.into_iter().zip(&paths) {
            file.persist(path).map_err(|error| error.error)?;
        }
        Ok(paths)
    }

    /// Writes each component into an unlinked file in `dir`. The caller owns
    /// publication; the returned files are positioned at the end of the ZIP.
    #[instrument(level = "info", skip_all)]
    pub fn write_unsigned_files(
        &self,
        options: ApkWriteOptions,
        dir: &Path,
    ) -> Result<Vec<(String, File)>> {
        let names = self.output_names()?;
        let files = self.write_components(
            options,
            names
                .iter()
                .map(|_| tempfile::tempfile_in(dir))
                .collect::<io::Result<_>>()?,
        )?;
        Ok(names.into_iter().zip(files).collect())
    }

    fn output_names(&self) -> Result<Vec<String>> {
        let mut used = HashSet::new();
        self.components
            .iter()
            .map(|component| {
                let name = component
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        invalid(
                            "apk output",
                            format!("{} has no UTF-8 file name", component.path().display()),
                        )
                    })?;
                if !used.insert(name) {
                    return Err(invalid(
                        "apk output",
                        format!("duplicate output file name {name}"),
                    ));
                }
                Ok(name.to_string())
            })
            .collect()
    }
}
