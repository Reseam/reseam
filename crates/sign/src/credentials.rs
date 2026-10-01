// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{Result, SignError, invalid};
use crate::key::SigningKey;

pub(crate) fn load_or_generate(key: &Path, cert: &Path) -> Result<SigningKey> {
    if pair_present(key, cert)? {
        return SigningKey::from_files(key, cert);
    }
    let generated = SigningKey::generate()?;
    save(&generated, key, cert)?;
    Ok(generated)
}

pub(crate) fn save(key: &SigningKey, key_path: &Path, cert_path: &Path) -> Result<()> {
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
        stage(key_path, key.pkcs8_der())?,
        stage(cert_path, key.certificate_der())?,
    ];
    for (path, file) in [key_path, cert_path].into_iter().zip(staged) {
        file.persist_noclobber(path)
            .map_err(|error| file_error("publishing credentials", path, error.error))?;
    }
    Ok(())
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>> {
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
