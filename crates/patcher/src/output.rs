// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::error::{PatcherError, Result};
use crate::log::{PatchLog, PatchScope};
use crate::options::PatchOptions;

/// What a patch works on while it rewrites output: the written APK, its
/// options and its log.
pub struct OutputContext<'a> {
    apk: &'a mut WrittenApk,
    scope: PatchScope,
}

impl<'a> OutputContext<'a> {
    pub(crate) fn new(apk: &'a mut WrittenApk, scope: PatchScope) -> Self {
        Self { apk, scope }
    }

    pub fn apk(&mut self) -> &mut WrittenApk {
        self.apk
    }

    pub fn log(&mut self) -> &mut PatchLog {
        &mut self.scope.log
    }

    pub fn options(&self) -> &PatchOptions {
        &self.scope.options
    }

    pub(crate) fn scope(&mut self) -> &mut PatchScope {
        &mut self.scope
    }
}

/// The unsigned APK files a run wrote, one per component, which patches that
/// rewrite output read and replace before the host signs them.
pub struct WrittenApk {
    components: Vec<WrittenComponent>,
    scratch: PathBuf,
    scratch_dirs: usize,
}

struct WrittenComponent {
    name: String,
    path: PathBuf,
}

impl WrittenApk {
    /// `components` pairs each component's name, as patches address it, with
    /// its written file, base first. The host removes `scratch` after the run.
    pub fn new(components: Vec<(String, PathBuf)>, scratch: PathBuf) -> Self {
        Self {
            components: components
                .into_iter()
                .map(|(name, path)| WrittenComponent { name, path })
                .collect(),
            scratch,
            scratch_dirs: 0,
        }
    }

    pub fn component_names(&self) -> impl Iterator<Item = &str> {
        self.components
            .iter()
            .map(|component| component.name.as_str())
    }

    /// The written file of `component`, or of the base APK when `None`.
    pub fn path(&self, component: Option<&str>) -> Result<&Path> {
        Ok(&self.component(component)?.path)
    }

    /// Moves `replacement` over the written file of `component`.
    pub fn replace(&mut self, component: Option<&str>, replacement: &Path) -> Result<()> {
        if !fs::metadata(replacement)?.is_file() {
            return Err(PatcherError::InvalidFile(format!(
                "{} is not a file",
                replacement.display()
            )));
        }
        let target = &self.component(component)?.path;
        match fs::rename(replacement, target) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
                fs::copy(replacement, target)?;
                Ok(fs::remove_file(replacement)?)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// A path for a new file called `name`, in a directory of its own inside
    /// the run's scratch directory.
    pub fn temp_file(&mut self, name: &str) -> Result<PathBuf> {
        let mut parts = Path::new(name).components();
        if !matches!(
            (parts.next(), parts.next()),
            (Some(Component::Normal(_)), None)
        ) {
            return Err(PatcherError::InvalidFile(format!(
                "temporary file name '{name}' is not a single path component"
            )));
        }
        self.scratch_dirs += 1;
        let dir = self.scratch.join(format!("rewrite-{}", self.scratch_dirs));
        fs::create_dir(&dir)?;
        Ok(dir.join(name))
    }

    fn component(&self, component: Option<&str>) -> Result<&WrittenComponent> {
        match component {
            None => Ok(self
                .components
                .first()
                .expect("a written APK has its base component")),
            Some(name) => self
                .components
                .iter()
                .find(|written| written.name == name)
                .ok_or_else(|| PatcherError::NotFound(format!("component {name}"))),
        }
    }
}
