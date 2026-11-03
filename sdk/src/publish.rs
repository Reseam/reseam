// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use reseam_model::PatchArtifact;
use tempfile::TempDir;
use tracing::info;

use crate::error::{HostError, Result, file_error};
use crate::staging::stage_file;

/// A written APK component at the path it is staged and signed at.
pub(crate) struct StagedComponent {
    name: String,
    staged: PathBuf,
    /// The output that holds it, which errors name.
    output: PathBuf,
}

/// A staged file and the destination it replaces when published.
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

/// How the signed components reach their destinations.
enum Packing {
    /// Each component is an artifact of its own.
    Files,
    /// The components go into the one artifact, a split archive.
    Archive,
}

/// Staged APKs that replace their destinations together, or not at all.
pub(crate) struct Publication {
    staging: TempDir,
    components: Vec<StagedComponent>,
    packing: Packing,
    artifacts: Vec<Artifact>,
}

impl StagedComponent {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn staged(&self) -> &Path {
        &self.staged
    }

    pub(crate) fn output(&self) -> &Path {
        &self.output
    }
}

impl Artifact {
    pub(crate) fn destination(&self) -> &Path {
        &self.destination
    }
}

impl Publication {
    pub(crate) fn new(
        staging: TempDir,
        files: &[(String, File)],
        output: &PatchArtifact,
    ) -> Result<Self> {
        let packing = match output {
            PatchArtifact::SingleFile { .. } | PatchArtifact::SplitDir { .. } => Packing::Files,
            PatchArtifact::SplitArchive { .. } => Packing::Archive,
        };
        let components: Vec<StagedComponent> = files
            .iter()
            .enumerate()
            .map(|(index, (name, _))| StagedComponent {
                name: name.clone(),
                staged: staging.path().join(format!("{index}.apk")),
                output: match output {
                    PatchArtifact::SplitDir { path } => Path::new(path).join(name),
                    PatchArtifact::SingleFile { .. } | PatchArtifact::SplitArchive { .. } => {
                        output.path().to_owned()
                    }
                },
            })
            .collect();
        let placements: Vec<(PathBuf, PathBuf)> = match packing {
            Packing::Files => components
                .iter()
                .map(|component| (component.output.clone(), component.staged.clone()))
                .collect(),
            Packing::Archive => vec![(output.path().to_owned(), staging.path().join("split.apks"))],
        };
        let artifacts = placements
            .into_iter()
            .map(|(destination, staged)| {
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
                    staged,
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
        Ok(Self {
            staging,
            components,
            packing,
            artifacts,
        })
    }

    pub(crate) fn components(&self) -> &[StagedComponent] {
        &self.components
    }

    pub(crate) fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    /// The directory holding the staged files until they are published.
    pub(crate) fn staging_dir(&self) -> &Path {
        self.staging.path()
    }

    /// Places each written file at its component's staged path.
    pub(crate) fn stage(&self, files: &[(String, File)]) -> Result<()> {
        for ((_, file), component) in files.iter().zip(&self.components) {
            stage_file(file, &component.staged)
                .map_err(|source| file_error("stage APK", &component.output, source))?;
        }
        Ok(())
    }

    /// Packs the signed components as the output asks, then replaces the
    /// destinations.
    pub(crate) fn publish(&mut self) -> Result<()> {
        if let (Packing::Archive, [archive]) = (&self.packing, self.artifacts.as_slice()) {
            let apks: Vec<(&str, &Path)> = self
                .components
                .iter()
                .map(|component| (component.name.as_str(), component.staged.as_path()))
                .collect();
            reseam_apk::write_split_archive(&apks, &archive.staged).map_err(|source| {
                HostError::WriteApk {
                    path: archive.destination.clone(),
                    source,
                }
            })?;
        }
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
