// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use reseam_apk::{ApkFile, ApkWriteOptions};
use reseam_patcher::output::WrittenApk;
use reseam_sign::{PreservedPairs, SigningKey};

use crate::error::{HostError, Result, file_error};
use crate::progress::RunProgress;
use crate::publish::Publication;
use reseam_model::{PatchArtifact, PatchOutput, PatchPhase, Problem, SigningIdentity};

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
        PatchOutput::SplitArchive { path } => PatchArtifact::SplitArchive { path: path.clone() },
    })
}

/// Unsigned output staged beside its destinations, waiting to be signed.
pub(crate) struct StagedOutput {
    publication: Publication,
    /// The signing block pairs each component's source carried, which its
    /// new signature keeps.
    preserved: Vec<PreservedPairs>,
    key: SigningKey,
}

/// Writes `apk` unsigned into a staging directory beside its destinations and
/// loads the key that will sign it.
pub(crate) fn stage_unsigned(
    apk: ApkFile,
    output: &PatchArtifact,
    signing: Option<&SigningIdentity>,
    progress: &mut RunProgress<'_>,
) -> Result<StagedOutput> {
    let (dir, key_stem) = match output {
        PatchArtifact::SingleFile { path } | PatchArtifact::SplitArchive { path } => (
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
    let preserved = apk
        .components()
        .iter()
        .map(|component| {
            let source = component.source().map_err(|source| HostError::Apk {
                path: component.path().to_owned(),
                source,
            })?;
            Ok(PreservedPairs::from_apk(&source)?)
        })
        .collect::<Result<_>>()?;
    let unsigned = progress
        .measure(PatchPhase::WriteUnsignedArtifacts, |_| {
            apk.write_unsigned_files(ApkWriteOptions::default(), dir)
        })
        .map_err(|source| HostError::WriteApk {
            path: output.path().to_owned(),
            source,
        })?;
    drop(apk);
    let publication = Publication::new(staging, &unsigned, output)?;
    let identity = signing.cloned().unwrap_or_else(|| SigningIdentity::Files {
        key: key_stem.with_extension("pk8").display().to_string(),
        cert: key_stem.with_extension("der").display().to_string(),
    });
    for artifact in publication.artifacts() {
        for path in identity_paths(&identity) {
            if same_destination(artifact.destination(), path)? {
                return Err(HostError::InvalidRequest(
                    "APK output would overwrite its signing identity",
                ));
            }
        }
    }
    let key = progress.measure(PatchPhase::LoadSigningKey, |_| load_key(&identity))?;
    publication.stage(&unsigned)?;
    Ok(StagedOutput {
        publication,
        preserved,
        key,
    })
}

impl StagedOutput {
    /// The staged files as patches that rewrite output see them, with the
    /// staging directory as their scratch space.
    pub(crate) fn written(&self) -> WrittenApk {
        WrittenApk::new(
            self.publication
                .components()
                .iter()
                .map(|component| (component.name().to_owned(), component.staged().to_owned()))
                .collect(),
            self.publication.staging_dir().to_owned(),
        )
    }

    pub(crate) fn sign_and_publish(mut self, progress: &mut RunProgress<'_>) -> Result<()> {
        progress.measure(PatchPhase::SignArtifacts, |_| {
            self.publication
                .components()
                .iter()
                .zip(&self.preserved)
                .try_for_each(|(component, preserved)| {
                    let file = File::options()
                        .read(true)
                        .write(true)
                        .open(component.staged())
                        .map_err(|source| {
                            file_error("open APK to sign", component.output(), source)
                        })?;
                    reseam_sign::v2::sign_file_in_place(&file, &self.key, preserved)?;
                    Ok::<_, HostError>(())
                })
        })?;
        self.publication.publish()
    }
}

fn identity_paths(identity: &SigningIdentity) -> Vec<&Path> {
    match identity {
        SigningIdentity::Files { key, cert } => vec![Path::new(key), Path::new(cert)],
        SigningIdentity::Pkcs12 { path, .. } => vec![Path::new(path)],
    }
}

fn load_key(identity: &SigningIdentity) -> Result<SigningKey> {
    Ok(match identity {
        SigningIdentity::Files { key, cert } => {
            SigningKey::load_or_generate(Path::new(key), Path::new(cert))?
        }
        SigningIdentity::Pkcs12 { path, password } => {
            let keystore = fs::read(path)
                .map_err(|source| file_error("read keystore", Path::new(path), source))?;
            SigningKey::from_pkcs12(&keystore, password)?
        }
    })
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
