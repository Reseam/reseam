// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use reseam_model::{PatchRef, Problem};
use thiserror::Error;

use crate::bundle::BUNDLE_FORMAT_VERSION;

#[derive(Debug, Error)]
pub enum PatcherError {
    #[error("bundle error: {0}")]
    Bundle(#[from] BundleError),

    /// A failure hosts tell apart, such as a bundle built for another engine.
    #[error(transparent)]
    Problem(#[from] Problem),

    #[error("extension DEX {file}: {reason}")]
    Extension {
        file: String,
        reason: ExtensionError,
    },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid file: {0}")]
    InvalidFile(String),

    #[error("dependency cycle: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join(" -> "))]
    DependencyCycle(Vec<PatchRef>),

    #[error(
        "missing dependency: patch {} depends on {}, which bundle '{}' does not declare",
        .0.patch, .0.dependency, .0.dependency.bundle
    )]
    MissingDependency(Box<Unresolved>),

    #[error(
        "missing bundle: patch {} depends on {}; load bundle '{}' alongside",
        .0.patch, .0.dependency, .0.dependency.bundle
    )]
    MissingBundle(Box<Unresolved>),

    #[error("patch {0} runs in this plan, but its code was not loaded")]
    NotLoaded(PatchRef),

    #[error("unknown patch: {0}")]
    UnknownPatch(String),

    #[error("invalid patch selection: {0}")]
    InvalidSelection(String),

    #[error("unknown option: patch {patch} has no option '{key}'")]
    UnknownOption { patch: PatchRef, key: String },

    #[error("invalid option value: {patch}.{key}: {reason}")]
    InvalidOptionValue {
        patch: PatchRef,
        key: String,
        reason: String,
    },

    #[error("missing required option: {patch}.{key}")]
    MissingRequiredOption { patch: PatchRef, key: String },

    #[error("app entry: {0}")]
    AppEntry(String),

    #[error("patch {patch} failed to rewrite the output: {reason}")]
    RewriteOutput { patch: PatchRef, reason: String },

    #[error("DEX error: {0}")]
    Dex(#[from] reseam_dex::DexError),

    #[error("Hermes error: {0}")]
    Hermes(#[from] reseam_hermes::HermesError),

    #[error("APK error: {0}")]
    Apk(#[from] reseam_apk::ApkError),

    #[error("native patch callback failed: {0}")]
    Bridge(String),

    #[error("JVM error: {0}")]
    Jvm(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// A patch and a dependency of it that no loaded patch is.
#[derive(Debug)]
pub struct Unresolved {
    pub patch: PatchRef,
    pub dependency: PatchRef,
}

/// Why a bundle cannot be packed, signed, opened or loaded.
#[derive(Debug, Error)]
pub enum BundleError {
    #[error("bundle name '{0}' must be lowercase letters, digits, and hyphens")]
    Name(String),
    #[error("unsupported bundle format_version {0} (supported: {BUNDLE_FORMAT_VERSION})")]
    FormatVersion(u32),
    #[error("invalid engine version `{0}`")]
    EngineVersion(String),
    #[error("bundle version must not be empty")]
    EmptyVersion,
    #[error("invalid patch identity {patch} in bundle {bundle}")]
    PatchIdentity { bundle: String, patch: PatchRef },
    #[error("bundle '{}' declares patch '{}' more than once", .0.bundle, .0.id)]
    DuplicatePatch(PatchRef),
    #[error("cannot open {}: {source}", path.display())]
    Archive {
        path: PathBuf,
        source: zip::result::ZipError,
    },
    #[error("required entry `{name}` cannot be read: {source}")]
    MissingEntry {
        name: &'static str,
        source: zip::result::ZipError,
    },
    #[error("control entry {name} exceeds {limit} bytes")]
    EntryTooLarge { name: &'static str, limit: u64 },
    #[error(
        "invalid mimetype marker (expected {})",
        crate::bundle::BUNDLE_MIMETYPE
    )]
    Mimetype,
    #[error("manifest signature: {0}")]
    Signature(&'static str),
    #[error("manifest.toml is not UTF-8")]
    ManifestEncoding,
    #[error("serialize manifest: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("unsafe payload path {0}")]
    UnsafePath(String),
    #[error("unsupported payload {0}")]
    UnsupportedPayload(String),
    #[error("non-UTF-8 payload name {0}")]
    PayloadName(String),
    #[error("payload {} must be a regular file", .0.display())]
    NotAFile(PathBuf),
    #[error("{} must be a directory", .0.display())]
    NotADirectory(PathBuf),
    #[error("no payload files in {}", .0.display())]
    NoPayload(PathBuf),
    #[error("payload {name}: {source}")]
    Payload {
        name: String,
        source: std::io::Error,
    },
    #[error("payload {0} changed while packing")]
    PayloadChanged(String),
    #[error("file {0} is undeclared or occurs more than once")]
    UndeclaredPayload(String),
    #[error("manifest hash for {0} is not a SHA-256 digest")]
    PayloadHash(String),
    #[error("hash mismatch for {0}")]
    HashMismatch(String),
    #[error("manifest declares files missing from the archive")]
    MissingPayload,
    #[error("{name} is not a valid jar: {source}")]
    Jar {
        name: String,
        source: zip::result::ZipError,
    },
    #[error("{name} is missing {missing}")]
    IncompleteJar { name: String, missing: JarPart },
    #[error("packing patch jars requires Kotlin support")]
    NoKotlin,
    #[error("patch jar {} has no declaration index: {source}; rebuild it with the Reseam Gradle plugin", jar.display())]
    MissingDeclarationIndex {
        jar: PathBuf,
        source: zip::result::ZipError,
    },
    #[error("declaration index in {} is too large", .0.display())]
    DeclarationIndexTooLarge(PathBuf),
    #[error("declaration index in {}: {source}", jar.display())]
    DeclarationIndex {
        jar: PathBuf,
        source: serde_json::Error,
    },
    #[error("indexed declaration class {0} is absent from the bundle jars")]
    UndeclaredClass(String),
    #[error("loaded patch metadata differs from the signed catalog; rebuild the bundle")]
    CatalogMismatch,
    #[error("signing key {}: {source}", path.display())]
    Key {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("signing key {} must be exactly 32 bytes", .0.display())]
    KeyLength(PathBuf),
    #[error("refusing to overwrite existing key at {}", .0.display())]
    KeyExists(PathBuf),
    #[error("generate signing seed")]
    KeyGeneration,
}

/// What a patch jar lacks to run on both the desktop JVM and ART.
#[derive(Debug, Clone, Copy)]
pub enum JarPart {
    Classes,
    Dex,
    Index,
}

impl std::fmt::Display for JarPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Classes => "JVM .class files",
            Self::Dex => "classes.dex; build patch jars as universal JVM/Android jars",
            Self::Index => "its patch declaration index; rebuild it with the Reseam Gradle plugin",
        })
    }
}

/// Why an extension DEX cannot join the classes patches link.
#[derive(Debug, Error)]
pub enum ExtensionError {
    #[error("{0}")]
    Dex(#[from] reseam_dex::DexError),
    #[error("class {index}: {source}")]
    Class {
        index: usize,
        source: reseam_dex::DexError,
    },
    #[error("defines {0}, a class of the app runtime the engine provides")]
    RuntimeClass(String),
    #[error("defines class {descriptor} differently than {other}")]
    Conflict { descriptor: String, other: String },
}

pub type Result<T> = std::result::Result<T, PatcherError>;
