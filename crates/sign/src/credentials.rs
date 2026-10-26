// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{Result, SignError, invalid};
use crate::key::SigningKey;

impl SigningKey {
    pub fn from_files(key_path: &Path, cert_path: &Path) -> Result<Self> {
        Self::from_pkcs8(&read(key_path)?, read(cert_path)?).map_err(|source| SignError::Identity {
            key: key_path.to_owned(),
            cert: cert_path.to_owned(),
            source: Box::new(source),
        })
    }

    /// Reuses existing credentials or generates them when both paths are absent.
    /// A partial pair is an error: restore the missing file or delete the other to regenerate.
    pub fn load_or_generate(key_path: &Path, cert_path: &Path) -> Result<Self> {
        if pair_present(key_path, cert_path)? {
            return Self::from_files(key_path, cert_path);
        }
        let generated = Self::generate()?;
        generated.save(key_path, cert_path)?;
        Ok(generated)
    }

    /// Publishes this pair without replacing either credential.
    /// Each file is staged in its destination directory before publication. An interruption
    /// between publications can leave a partial pair that must be restored or removed.
    /// Both parents must exist; the certificate can live in another directory/filesystem.
    pub fn save(&self, key_path: &Path, cert_path: &Path) -> Result<()> {
        if pair_present(key_path, cert_path)? {
            return Err(file_error(
                "saving credentials",
                key_path,
                std::io::ErrorKind::AlreadyExists.into(),
            ));
        }
        if key_path == cert_path {
            return Err(invalid("signing paths", "paths must differ"));
        }
        let staged = [
            stage(key_path, self.pkcs8_der())?,
            stage(cert_path, self.certificate_der())?,
        ];
        for (path, file) in [key_path, cert_path].into_iter().zip(staged) {
            file.persist_noclobber(path)
                .map_err(|error| file_error("publishing credentials", path, error.error))?;
        }
        Ok(())
    }
}

fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| file_error("reading credentials", path, source))
}
fn pair_present(key: &Path, cert: &Path) -> Result<bool> {
    let (missing, existing) = match (present(key)?, present(cert)?) {
        (true, true) => return Ok(true),
        (false, false) => return Ok(false),
        (true, false) => (cert, key),
        (false, true) => (key, cert),
    };
    Err(SignError::PartialPair {
        missing: missing.to_owned(),
        existing: existing.to_owned(),
    })
}

fn present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(file_error("checking credentials", path, source)),
    }
}

fn stage(path: &Path, bytes: &[u8]) -> Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|source| file_error("staging credentials", path, source))?;
    file.write_all(bytes)
        .and_then(|()| file.as_file().sync_all())
        .map_err(|source| file_error("writing credentials", path, source))?;
    Ok(file)
}

fn file_error(operation: &'static str, path: &Path, source: std::io::Error) -> SignError {
    SignError::File {
        operation,
        path: path.to_owned(),
        source,
    }
}
