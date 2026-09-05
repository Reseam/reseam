// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use reseam_apk::{ApkFile, ApkWriteOptions};
use reseam_sign::{GeneratedKey, SigningKey};
use tracing::info;

use crate::dto::{PatchOutput, SigningKeyFiles};
use crate::metrics::{PatchPhase, PatchProfiler};

/// Writes every component unsigned into the output directory, signs each in
/// place, and only then links it under its final name.
pub(crate) fn write_signed(
    mut apk: ApkFile,
    output: &PatchOutput,
    signing: Option<&SigningKeyFiles>,
    profiler: &mut PatchProfiler,
) -> Result<()> {
    let (dir, key_stem): (&Path, PathBuf) = match output {
        PatchOutput::SingleFile { path } => (
            path.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
            path.with_extension(""),
        ),
        PatchOutput::SplitDir { path } => (path, path.join("reseam")),
    };
    std::fs::create_dir_all(dir)
        .with_context(|| format!("failed to create output directory {}", dir.display()))?;

    let unsigned = profiler
        .measure(PatchPhase::WriteUnsignedArtifacts, || {
            apk.write_unsigned_files(ApkWriteOptions::default(), dir)
        })
        .context("failed to write patched APK")?;
    let key = profiler.measure(PatchPhase::LoadSigningKey, || {
        signing_key(signing, &key_stem)
    })?;
    profiler.measure(PatchPhase::SignArtifacts, || {
        unsigned.iter().try_for_each(|(name, file)| {
            let destination = match output {
                PatchOutput::SingleFile { path } => path.clone(),
                PatchOutput::SplitDir { path } => path.join(name),
            };
            sign_into_place(file, &destination, &key)
        })
    })
}

/// The caller's key pair, or one generated beside the output on first use.
fn signing_key(files: Option<&SigningKeyFiles>, default_stem: &Path) -> Result<SigningKey> {
    let (key, cert) = match files {
        Some(files) => (files.key.clone(), files.cert.clone()),
        None => (
            default_stem.with_extension("pk8"),
            default_stem.with_extension("der"),
        ),
    };
    if !(key.exists() && cert.exists()) {
        GeneratedKey::generate()
            .context("failed to generate signing key")?
            .save(&key, &cert)
            .context("failed to save signing key")?;
    }
    SigningKey::from_files(&key, &cert).context("failed to load signing key")
}

fn sign_into_place(output: &File, path: &Path, key: &SigningKey) -> Result<()> {
    reseam_sign::v2::sign_file_in_place(output, key).context("v2 signing failed")?;
    place_file(output, path).with_context(|| format!("failed to write {}", path.display()))?;
    info!(path = %path.display(), "patched APK written");
    Ok(())
}

/// Links an unlinked temp file to `path`, falling back to a copy where the
/// file system cannot link anonymous files.
fn place_file(file: &File, path: &Path) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let source = std::ffi::CString::new(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
    let target = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())?;
    // SAFETY: both paths are valid C strings and linkat only creates a directory entry.
    let rc = unsafe {
        libc::linkat(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::AT_SYMLINK_FOLLOW,
        )
    };
    if rc == 0 {
        return Ok(());
    }
    let mut reader = std::io::BufReader::new(file);
    reader.seek(std::io::SeekFrom::Start(0))?;
    let mut writer = std::io::BufWriter::new(File::create(path)?);
    std::io::copy(&mut reader, &mut writer)?;
    writer.flush()
}
