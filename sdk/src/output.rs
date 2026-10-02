// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, File};
use std::io::{self, Seek, Write};
use std::path::{Path, PathBuf};

use reseam_apk::{ApkFile, ApkWriteOptions};
use reseam_sign::SigningKey;
use tempfile::TempDir;
use tracing::info;

use crate::error::{HostError, Result, file_error};
use crate::metrics::{PatchPhase, PatchProfiler};
use crate::{PatchArtifact, SigningKeyFiles};

pub(crate) fn write_signed(
    apk: ApkFile,
    output: &PatchArtifact,
    signing: Option<&SigningKeyFiles>,
    profiler: &mut PatchProfiler,
) -> Result<()> {
    let (dir, key_stem) = match output {
        PatchArtifact::SingleFile { path } => (
            Path::new(path)
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
            Path::new(path).with_extension(""),
        ),
        PatchArtifact::SplitDir { path } => (Path::new(path), Path::new(path).join("reseam")),
    };
    fs::create_dir_all(dir).map_err(|source| file_error("create output directory", dir, source))?;
    let staging = tempfile::Builder::new()
        .prefix(".reseam-")
        .tempdir_in(dir)
        .map_err(|source| file_error("create output staging directory", dir, source))?;
    let unsigned = profiler
        .measure(PatchPhase::WriteUnsignedArtifacts, || {
            apk.write_unsigned_files(ApkWriteOptions::default(), dir)
        })
        .map_err(|source| HostError::WriteApk {
            path: output.path().to_owned(),
            source,
        })?;
    drop(apk);
    let mut publication = Publication::new(staging, &unsigned, output)?;
    let (key_path, cert_path) = signing_paths(signing, &key_stem);
    for artifact in &publication.artifacts {
        for identity in [&key_path, &cert_path] {
            if same_destination(&artifact.destination, identity)? {
                return Err(HostError::InvalidRequest(
                    "APK output would overwrite its signing identity",
                ));
            }
        }
    }
    let key = profiler.measure(PatchPhase::LoadSigningKey, || {
        SigningKey::load_or_generate(&key_path, &cert_path)
    })?;
    profiler.measure(PatchPhase::SignArtifacts, || {
        unsigned
            .iter()
            .zip(&publication.artifacts)
            .try_for_each(|((_, file), artifact)| {
                reseam_sign::v2::sign_file_in_place(file, &key)?;
                stage_file(file, &artifact.staged)
                    .map_err(|source| file_error("stage signed APK", &artifact.destination, source))
            })
    })?;
    publication.publish()
}

fn signing_paths(files: Option<&SigningKeyFiles>, default_stem: &Path) -> (PathBuf, PathBuf) {
    match files {
        Some(files) => (PathBuf::from(&files.key), PathBuf::from(&files.cert)),
        None => (
            default_stem.with_extension("pk8"),
            default_stem.with_extension("der"),
        ),
    }
}

fn same_destination(output: &Path, identity: &Path) -> Result<bool> {
    if output == identity {
        return Ok(true);
    }
    let resolved_identity = match reseam_storage::canonicalize(identity) {
        Ok(path) => Some(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(file_error("resolve signing file", identity, error)),
    };
    let identity = resolved_identity.as_deref().unwrap_or(identity);
    let parent = |path: &Path| -> Result<Option<PathBuf>> {
        let dir = path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        match reseam_storage::canonicalize(dir) {
            Ok(dir) => Ok(Some(dir)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(file_error(
                "resolve output or signing directory",
                dir,
                error,
            )),
        }
    };
    #[cfg(windows)]
    let same_name = output
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        == identity
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase());
    #[cfg(not(windows))]
    let same_name = output.file_name() == identity.file_name();
    if !same_name {
        return Ok(false);
    }
    let (output_parent, identity_parent) = (parent(output)?, parent(identity)?);
    Ok(output_parent.is_some() && output_parent == identity_parent)
}

struct Artifact {
    destination: PathBuf,
    staged: PathBuf,
    state: PublicationState,
}

enum PublicationState {
    Staged,
    BackedUp(PathBuf),
    Published(Option<PathBuf>),
}

struct Publication {
    staging: TempDir,
    artifacts: Vec<Artifact>,
}

impl Publication {
    fn new(staging: TempDir, files: &[(String, File)], output: &PatchArtifact) -> Result<Self> {
        let artifacts = files
            .iter()
            .enumerate()
            .map(|(index, (name, _))| {
                let destination = match output {
                    PatchArtifact::SingleFile { path } => PathBuf::from(path),
                    PatchArtifact::SplitDir { path } => Path::new(path).join(name),
                };
                match fs::symlink_metadata(&destination) {
                    Ok(metadata) if metadata.is_dir() => {
                        return Err(file_error(
                            "replace output",
                            &destination,
                            io::ErrorKind::IsADirectory.into(),
                        ));
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(file_error("inspect output", &destination, error)),
                }
                Ok(Artifact {
                    destination,
                    staged: staging.path().join(format!("{index}.apk")),
                    state: PublicationState::Staged,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let destinations: std::collections::HashSet<_> = artifacts
            .iter()
            .map(|artifact| {
                #[cfg(windows)]
                {
                    artifact.destination.to_string_lossy().to_lowercase()
                }
                #[cfg(not(windows))]
                {
                    artifact.destination.clone()
                }
            })
            .collect();
        if destinations.len() != artifacts.len() {
            return Err(HostError::InvalidRequest(
                "APK components have colliding output names",
            ));
        }
        Ok(Self { staging, artifacts })
    }

    fn publish(&mut self) -> Result<()> {
        if let Err(source) = self.commit() {
            if let Err(rollback) = self.rollback() {
                let recovery = self.staging.path().to_owned();
                self.staging.disable_cleanup(true);
                return Err(HostError::Publication {
                    source: Box::new(source),
                    rollback: Box::new(rollback),
                    recovery,
                });
            }
            return Err(source);
        }
        for artifact in &self.artifacts {
            info!(path = %artifact.destination.display(), "patched APK written");
        }
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        for (index, artifact) in self.artifacts.iter_mut().enumerate() {
            let backup = self.staging.path().join(format!("{index}.previous"));
            match fs::rename(&artifact.destination, &backup) {
                Ok(()) => artifact.state = PublicationState::BackedUp(backup.clone()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(file_error(
                        "retain previous output",
                        &artifact.destination,
                        error,
                    ));
                }
            }
            fs::rename(&artifact.staged, &artifact.destination).map_err(|source| {
                file_error("publish signed APK", &artifact.destination, source)
            })?;
            let previous = match std::mem::replace(&mut artifact.state, PublicationState::Staged) {
                PublicationState::BackedUp(path) => Some(path),
                PublicationState::Staged => None,
                PublicationState::Published(_) => unreachable!("each artifact is published once"),
            };
            artifact.state = PublicationState::Published(previous);
        }
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        let mut failure = None;
        for artifact in self.artifacts.iter_mut().rev() {
            let restore = || -> Result<()> {
                let previous = match &artifact.state {
                    PublicationState::Staged => return Ok(()),
                    PublicationState::BackedUp(path) => Some(path),
                    PublicationState::Published(previous) => {
                        fs::remove_file(&artifact.destination).map_err(|source| {
                            file_error(
                                "remove unpublished replacement",
                                &artifact.destination,
                                source,
                            )
                        })?;
                        previous.as_ref()
                    }
                };
                if let Some(previous) = previous {
                    fs::rename(previous, &artifact.destination).map_err(|source| {
                        file_error("restore previous output", &artifact.destination, source)
                    })?;
                }
                Ok(())
            };
            match restore() {
                Ok(()) => artifact.state = PublicationState::Staged,
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

fn stage_file(file: &File, path: &Path) -> io::Result<()> {
    match link_anonymous(file, path) {
        Ok(()) => return Ok(()),
        Err(error) if matches!(error.kind(), io::ErrorKind::AlreadyExists) => {
            return Err(error);
        }
        Err(_) => {}
    }
    let mut reader = io::BufReader::new(file);
    reader.seek(io::SeekFrom::Start(0))?;
    let mut writer = io::BufWriter::new(File::options().write(true).create_new(true).open(path)?);
    io::copy(&mut reader, &mut writer)?;
    writer.flush()
}

#[cfg(target_os = "linux")]
fn link_anonymous(file: &File, path: &Path) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let source = std::ffi::CString::new(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
    let target = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())?;
    // SAFETY: both paths are valid C strings; linkat only creates a directory entry.
    let result = unsafe {
        libc::linkat(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::AT_SYMLINK_FOLLOW,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "linux"))]
fn link_anonymous(_file: &File, _path: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
