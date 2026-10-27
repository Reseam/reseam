// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use reseam_apk::{ApkFile, ApkWriteOptions};
use reseam_patcher::output::WrittenApk;
use reseam_sign::SigningKey;

use crate::error::{HostError, Result, file_error};
use crate::metrics::PatchProfiler;
use crate::publish::Publication;
use reseam_model::{PatchArtifact, PatchOutput, PatchPhase, Problem, SigningKeyFiles};

/// Resolves automatic output using the input component count.
/// Explicit file output requires exactly one component; directories are preserved.
pub(crate) fn resolve_output(output: &PatchOutput, components: usize) -> Result<PatchArtifact> {
    Ok(match output {
        PatchOutput::Auto { path } if components == 1 => PatchArtifact::SingleFile {
            path: format!("{path}.apk"),
        },
        PatchOutput::Auto { path } | PatchOutput::SplitDir { path } => {
            PatchArtifact::SplitDir { path: path.clone() }
        }
        PatchOutput::SingleFile { path } => {
            if components != 1 {
                return Err(Problem::SingleFileComponents { components }.into());
            }
            PatchArtifact::SingleFile { path: path.clone() }
        }
    })
}

/// Unsigned output staged beside its destinations, waiting to be signed.
pub(crate) struct StagedOutput {
    publication: Publication,
    components: Vec<String>,
    key: SigningKey,
}

/// Writes `apk` unsigned into a staging directory beside its destinations and
/// loads the key that will sign it.
pub(crate) fn stage_unsigned(
    apk: ApkFile,
    output: &PatchArtifact,
    signing: Option<&SigningKeyFiles>,
    profiler: &mut PatchProfiler,
) -> Result<StagedOutput> {
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
    let components = apk
        .components()
        .iter()
        .map(|component| component.name().to_owned())
        .collect();
    let unsigned = profiler
        .measure(PatchPhase::WriteUnsignedArtifacts, || {
            apk.write_unsigned_files(ApkWriteOptions::default(), dir)
        })
        .map_err(|source| HostError::WriteApk {
            path: output.path().to_owned(),
            source,
        })?;
    drop(apk);
    let publication = Publication::new(staging, &unsigned, output)?;
    let (key_path, cert_path) = signing_paths(signing, &key_stem);
    for artifact in publication.artifacts() {
        for identity in [&key_path, &cert_path] {
            if same_destination(artifact.destination(), identity)? {
                return Err(HostError::InvalidRequest(
                    "APK output would overwrite its signing identity",
                ));
            }
        }
    }
    let key = profiler.measure(PatchPhase::LoadSigningKey, || {
        SigningKey::load_or_generate(&key_path, &cert_path)
    })?;
    publication.stage(&unsigned)?;
    Ok(StagedOutput {
        publication,
        components,
        key,
    })
}

impl StagedOutput {
    /// The staged files as patches that rewrite output see them, with the
    /// staging directory as their scratch space.
    pub(crate) fn written(&self) -> WrittenApk {
        WrittenApk::new(
            self.components
                .iter()
                .cloned()
                .zip(
                    self.publication
                        .artifacts()
                        .iter()
                        .map(|artifact| artifact.staged().to_owned()),
                )
                .collect(),
            self.publication.staging_dir().to_owned(),
        )
    }

    pub(crate) fn sign_and_publish(mut self, profiler: &mut PatchProfiler) -> Result<()> {
        profiler.measure(PatchPhase::SignArtifacts, || {
            self.publication
                .artifacts()
                .iter()
                .try_for_each(|artifact| {
                    let file = File::options()
                        .read(true)
                        .write(true)
                        .open(artifact.staged())
                        .map_err(|source| {
                            file_error("open APK to sign", artifact.destination(), source)
                        })?;
                    reseam_sign::v2::sign_file_in_place(&file, &self.key)?;
                    Ok::<_, HostError>(())
                })
        })?;
        self.publication.publish()
    }
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
