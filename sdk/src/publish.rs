// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Seek, Write};
use std::path::{Path, PathBuf};

use reseam_model::PatchArtifact;
use tempfile::TempDir;
use tracing::info;

use crate::error::{HostError, Result, file_error};

/// A staged APK and the destination it replaces when published.
pub(crate) struct Artifact {
    destination: PathBuf,
    staged: PathBuf,
    state: PublicationState,
}

enum PublicationState {
    Staged,
    BackedUp(PathBuf),
    Published(Option<PathBuf>),
}

/// Staged APKs that replace their destinations together, or not at all.
pub(crate) struct Publication {
    staging: TempDir,
    artifacts: Vec<Artifact>,
}

impl Artifact {
    pub(crate) fn destination(&self) -> &Path {
        &self.destination
    }

    pub(crate) fn staged(&self) -> &Path {
        &self.staged
    }
}

impl Publication {
    pub(crate) fn new(
        staging: TempDir,
        files: &[(String, File)],
        output: &PatchArtifact,
    ) -> Result<Self> {
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
        let destinations: HashSet<_> = artifacts
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

    pub(crate) fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    /// The directory holding the staged files until they are published.
    pub(crate) fn staging_dir(&self) -> &Path {
        self.staging.path()
    }

    /// Places each written file at its artifact's staged path.
    pub(crate) fn stage(&self, files: &[(String, File)]) -> Result<()> {
        for ((_, file), artifact) in files.iter().zip(&self.artifacts) {
            stage_file(file, &artifact.staged)
                .map_err(|source| file_error("stage APK", &artifact.destination, source))?;
        }
        Ok(())
    }

    pub(crate) fn publish(&mut self) -> Result<()> {
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
