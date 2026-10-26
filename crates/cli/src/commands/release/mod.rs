// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod forge;
mod http;
mod index;
mod notes;
mod s3;

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use reseam_model::{InspectRequest, InspectResponse, Trust};
use reseam_sdk::inspect;
use semver::Version;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tracing::info;
use url::Url;

use self::forge::Forge;
use self::http::file_name;
use self::index::{Entry, Index, Publisher};
use self::s3::{Caching, S3};
use crate::app::{
    ReleaseArgs, ReleaseFilesCommand, ReleaseManagerCommand, ReleasePatchesCommand, Target,
};

const PATCHES_INDEX: &str = "patches.json";
const MANAGER_INDEX: &str = "manager.json";

pub fn run_release_patches(command: &ReleasePatchesCommand) -> Result<()> {
    let InspectResponse {
        mut bundles,
        patches,
        ..
    } = inspect(&InspectRequest {
        apk_path: None,
        split_paths: Vec::new(),
        bundle_paths: vec![command.bundle.display().to_string()],
        trust: Trust::default(),
    })?;
    let info = bundles.remove(0);
    if let Some(problem) = info.problem {
        bail!(
            "failed to open bundle {}: {problem}",
            command.bundle.display()
        );
    }
    ensure!(
        !info.version.is_empty(),
        "{} has no version; pack it with --version",
        command.bundle.display()
    );
    let release = Release::open(&command.release)?;
    let previous = release.store.previous_index(PATCHES_INDEX)?;
    let homepage = command
        .homepage
        .clone()
        .or_else(|| {
            previous
                .as_ref()
                .and_then(|index| index.homepage().map(str::to_owned))
        })
        .or_else(|| release.forge().map(|forge| forge.page().to_string()));
    let publisher = Publisher {
        name: info.name.clone(),
        author: info.author.clone(),
        description: info.description.clone(),
        homepage,
        public_key: Some(info.public_key),
    };
    let prerelease = is_prerelease(&info.version);
    let entry = Entry {
        version: info.version.clone(),
        created_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        description: release.notes.clone(),
        download_url: release
            .store
            .download_url(&release.args.tag, file_name(&command.bundle)?)?
            .to_string(),
        prerelease,
        patches: Some(patches.into_iter().map(|patch| patch.spec).collect()),
    };
    let staged = Staged::index(PATCHES_INDEX, &Index::add(previous, publisher, entry)?)?;
    let body = match &release.store {
        Store::Forge(forge) => format!(
            "[Add to Reseam Manager](https://reseam.app/add/?url={})\n\n{}",
            url::form_urlencoded::byte_serialize(forge.page().as_str().as_bytes())
                .collect::<String>(),
            release.notes
        ),
        Store::Files(_) => release.notes.clone(),
    };
    release.publish(&body, prerelease, &[&command.bundle], Some(&staged.path))
}

pub fn run_release_manager(command: &ReleaseManagerCommand) -> Result<()> {
    ensure!(!command.name.trim().is_empty(), "--name must not be empty");
    let files = files_in(&command.dir)?;
    ensure!(
        files
            .iter()
            .all(|path| path.file_name() != Some(MANAGER_INDEX.as_ref())),
        "{} already holds a {MANAGER_INDEX}",
        command.dir.display()
    );
    let release = Release::open(&command.release)?;
    let version = release.args.tag.trim_start_matches('v').to_owned();
    let prerelease = is_prerelease(&version);
    let publisher = Publisher {
        name: command.name.clone(),
        author: command.author.clone(),
        description: command.summary.clone(),
        homepage: None,
        public_key: None,
    };
    let entry = Entry {
        version,
        created_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        description: release.notes.clone(),
        download_url: command.download_url.to_string(),
        prerelease,
        patches: None,
    };
    let previous = release.store.previous_index(MANAGER_INDEX)?;
    let staged = Staged::index(MANAGER_INDEX, &Index::add(previous, publisher, entry)?)?;
    let files: Vec<_> = files.iter().map(PathBuf::as_path).collect();
    release.publish(&release.notes, prerelease, &files, Some(&staged.path))
}

pub fn run_release_files(command: &ReleaseFilesCommand) -> Result<()> {
    let files = files_in(&command.dir)?;
    let release = Release::open(&command.release)?;
    let files: Vec<_> = files.iter().map(PathBuf::as_path).collect();
    let prerelease = is_prerelease(&release.args.tag);
    release.publish(&release.notes, prerelease, &files, None)
}

struct Release<'a> {
    args: &'a ReleaseArgs,
    store: Store,
    announcement: Option<Forge>,
    notes: String,
}

impl<'a> Release<'a> {
    fn open(args: &'a ReleaseArgs) -> Result<Self> {
        let store = Store::open(args)?;
        let announcement = if args.forge_release {
            Some(Forge::from_env()?)
        } else {
            None
        };
        let notes = match &args.notes {
            Some(path) => std::fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?,
            None => notes::commit_subjects(&args.tag)?,
        };
        Ok(Self {
            args,
            store,
            announcement,
            notes,
        })
    }

    fn forge(&self) -> Option<&Forge> {
        match &self.store {
            Store::Forge(forge) => Some(forge),
            Store::Files(_) => self.announcement.as_ref(),
        }
    }

    fn publish(
        &self,
        body: &str,
        prerelease: bool,
        files: &[&Path],
        index: Option<&Path>,
    ) -> Result<()> {
        let tag = &self.args.tag;
        match &self.store {
            Store::Forge(forge) => {
                let assets: Vec<_> = files.iter().copied().chain(index).collect();
                forge.publish(tag, body, prerelease, &assets)
            }
            Store::Files(store) => {
                store.upload(tag, files, index)?;
                if let Some(keep) = self.args.keep {
                    store.prune(keep)?;
                }
                match &self.announcement {
                    Some(forge) => forge.publish(tag, body, prerelease, &[]),
                    None => Ok(()),
                }
            }
        }
    }
}

enum Store {
    Forge(Forge),
    Files(Files),
}

impl Store {
    fn open(args: &ReleaseArgs) -> Result<Self> {
        let public_url = args.public_url.clone();
        Ok(match &args.to {
            Target::Forge => {
                ensure!(
                    public_url.is_none(),
                    "--public-url is for s3 and folder targets"
                );
                ensure!(
                    !args.forge_release,
                    "--forge-release is for s3 and folder targets"
                );
                ensure!(args.keep.is_none(), "--keep is for s3 and folder targets");
                Self::Forge(Forge::from_env()?)
            }
            Target::S3 { bucket, prefix } => Self::Files(Files::S3 {
                s3: Box::new(S3::from_env(bucket)?),
                prefix: prefix.clone(),
                public_url,
            }),
            Target::Directory(root) => Self::Files(Files::Directory {
                root: root.clone(),
                public_url,
            }),
        })
    }

    fn previous_index(&self, name: &str) -> Result<Option<Index>> {
        let json = match self {
            Self::Forge(forge) => forge.latest_asset(name)?,
            Self::Files(files) => files.latest(name)?,
        };
        json.as_deref().map(Index::parse).transpose()
    }

    fn download_url(&self, tag: &str, name: &str) -> Result<Url> {
        match self {
            Self::Forge(forge) => Ok(forge.asset_url(tag, name)),
            Self::Files(files) => files.download_url(tag, name),
        }
    }
}

enum Files {
    S3 {
        s3: Box<S3>,
        prefix: String,
        public_url: Option<Url>,
    },
    Directory {
        root: PathBuf,
        public_url: Option<Url>,
    },
}

impl Files {
    fn latest(&self, name: &str) -> Result<Option<String>> {
        match self {
            Self::S3 { s3, prefix, .. } => s3.get(&key(prefix, [name])),
            Self::Directory { root, .. } => match std::fs::read_to_string(root.join(name)) {
                Ok(json) => Ok(Some(json)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error).context("failed to read the published index"),
            },
        }
    }

    fn download_url(&self, tag: &str, name: &str) -> Result<Url> {
        let (public_url, prefix) = match self {
            Self::S3 {
                public_url, prefix, ..
            } => (public_url, prefix.as_str()),
            Self::Directory { public_url, .. } => (public_url, ""),
        };
        let mut url = public_url
            .clone()
            .context("--public-url is required to link the release from its index")?;
        url.path_segments_mut()
            .expect("http URLs have path segments")
            .pop_if_empty()
            .extend(key(prefix, [tag, name]).split('/'));
        Ok(url)
    }

    fn upload(&self, tag: &str, files: &[&Path], index: Option<&Path>) -> Result<()> {
        match self {
            Self::S3 { s3, prefix, .. } => {
                for path in files.iter().copied().chain(index) {
                    s3.put(
                        &key(prefix, [tag, file_name(path)?]),
                        path,
                        Caching::Release,
                    )?;
                }
                // The latest index goes last, so it never points at files still uploading.
                if let Some(index) = index {
                    s3.put(&key(prefix, [file_name(index)?]), index, Caching::Latest)?;
                }
                info!(tag, prefix, "uploaded the release to S3");
            }
            Self::Directory { root, .. } => {
                let folder = root.join(tag);
                std::fs::create_dir_all(&folder)
                    .with_context(|| format!("failed to create {}", folder.display()))?;
                for path in files.iter().copied().chain(index) {
                    copy(path, &folder.join(file_name(path)?))?;
                }
                if let Some(index) = index {
                    copy(index, &root.join(file_name(index)?))?;
                }
                info!(tag, folder = %folder.display(), "wrote the release");
            }
        }
        Ok(())
    }

    fn prune(&self, keep: NonZeroUsize) -> Result<()> {
        let tags = match self {
            Self::S3 { s3, prefix, .. } => s3.folders(&key(prefix, [""]))?,
            Self::Directory { root, .. } => {
                let mut tags = Vec::new();
                for entry in std::fs::read_dir(root)? {
                    let entry = entry?;
                    if entry.file_type()?.is_dir()
                        && let Ok(name) = entry.file_name().into_string()
                    {
                        tags.push(name);
                    }
                }
                tags
            }
        };
        let mut releases: Vec<_> = tags
            .into_iter()
            .filter_map(|tag| Some((Version::parse(tag.trim_start_matches('v')).ok()?, tag)))
            .collect();
        releases.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, tag) in releases.into_iter().skip(keep.get()) {
            match self {
                Self::S3 { s3, prefix, .. } => s3.delete_all(&key(prefix, [tag.as_str(), ""]))?,
                Self::Directory { root, .. } => std::fs::remove_dir_all(root.join(&tag))?,
            }
            info!(tag, "pruned an old release");
        }
        Ok(())
    }
}

struct Staged {
    _folder: tempfile::TempDir,
    path: PathBuf,
}

impl Staged {
    fn index(name: &str, index: &Index) -> Result<Self> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join(name);
        std::fs::write(&path, index.to_json()?)?;
        Ok(Self {
            _folder: folder,
            path,
        })
    }
}

fn key<'a>(prefix: &'a str, parts: impl IntoIterator<Item = &'a str>) -> String {
    std::iter::once(prefix)
        .filter(|prefix| !prefix.is_empty())
        .chain(parts)
        .collect::<Vec<_>>()
        .join("/")
}

fn files_in(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))?
    {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    ensure!(!files.is_empty(), "{} holds no files", dir.display());
    files.sort();
    Ok(files)
}

/// Copies into a new file, so the published copy gets the default permissions a web server can read.
fn copy(from: &Path, to: &Path) -> Result<()> {
    let context = || format!("failed to copy {} to {}", from.display(), to.display());
    let mut source = std::fs::File::open(from).with_context(context)?;
    let mut target = std::fs::File::create(to).with_context(context)?;
    std::io::copy(&mut source, &mut target).with_context(context)?;
    Ok(())
}

fn is_prerelease(version: &str) -> bool {
    Version::parse(version.trim_start_matches('v')).is_ok_and(|version| !version.pre.is_empty())
}
