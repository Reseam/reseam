// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Every class of a bundle's payload as hashed declarations with what each
//! refers to, and the payload's other files with their hashes.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;

use reseam_dex::{
    AccessFlags, ClassDeclarations, Declaration, DexFile, Loading, MemberDeclaration, MethodKind,
    ParseOptions, References, class_declarations,
};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::bundle::PATCH_INDEX;
use crate::bundle::payload::{PayloadKind, payload_kind};
use crate::bundle::stage::Staged;
use crate::error::{BundleError, JarPart, Result};

/// Files by name, with their SHA-256.
pub(super) type HashedFiles = Vec<(String, [u8; 32])>;

/// Where code runs: patch jars in the engine, extensions in the patched app.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Side {
    Patch,
    Extension,
}

impl Side {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Patch => "patch",
            Self::Extension => "extension",
        }
    }
}

pub(super) struct Graph {
    pub(super) patch: HashMap<String, Class>,
    pub(super) extension: HashMap<String, Class>,
    /// The files in each patch jar other than its code.
    pub(super) jar_files: Vec<HashedFiles>,
    pub(super) resources: HashedFiles,
}

pub(super) struct Class {
    /// The patch jar the class comes from; extension classes have none.
    pub(super) jar: Option<usize>,
    pub(super) supertypes: Vec<String>,
    pub(super) header: Node,
    /// Keyed by the descriptor after `->`: `name:I`, `name(I)V`.
    pub(super) fields: BTreeMap<String, Member>,
    pub(super) methods: BTreeMap<String, Member>,
}

pub(super) struct Member {
    flags: AccessFlags,
    pub(super) node: Node,
}

pub(super) struct Node {
    pub(super) hash: [u8; 32],
    pub(super) references: References,
}

impl Graph {
    /// Payload order decides which definition of a class several files share counts.
    pub(super) fn read(payload: &BTreeMap<String, Staged>) -> Result<Self> {
        let mut graph = Self {
            patch: HashMap::new(),
            extension: HashMap::new(),
            jar_files: Vec::new(),
            resources: Vec::new(),
        };
        let options = ParseOptions {
            classes: Loading::Deferred,
            ..ParseOptions::default()
        };
        for (name, staged) in payload {
            let dex_error = |source| BundleError::PayloadDex {
                name: name.clone(),
                source,
            };
            match payload_kind(name)? {
                PayloadKind::Jar => {
                    let (dex, files) = read_jar(name, &staged.path)?;
                    let jar = graph.jar_files.len();
                    graph.jar_files.push(files);
                    let dex = reseam_dex::parse_owned(dex, options).map_err(dex_error)?;
                    add(&mut graph.patch, &dex, Some(jar)).map_err(dex_error)?;
                }
                PayloadKind::Dex => {
                    let dex = reseam_dex::parse_file(&staged.path, options).map_err(dex_error)?;
                    add(&mut graph.extension, &dex, None).map_err(dex_error)?;
                }
                PayloadKind::Resource => graph.resources.push((name.clone(), staged.hash)),
            }
        }
        Ok(graph)
    }

    pub(super) fn classes(&self, side: Side) -> &HashMap<String, Class> {
        match side {
            Side::Patch => &self.patch,
            Side::Extension => &self.extension,
        }
    }
}

fn add(
    classes: &mut HashMap<String, Class>,
    dex: &DexFile,
    jar: Option<usize>,
) -> reseam_dex::Result<()> {
    for class_idx in 0..dex.classes().len() {
        let class = class_declarations(dex, class_idx)?;
        if !classes.contains_key(&class.descriptor) {
            classes.insert(class.descriptor.clone(), Class::new(class, jar));
        }
    }
    Ok(())
}

impl Class {
    fn new(class: ClassDeclarations, jar: Option<usize>) -> Self {
        let members = |members: Vec<MemberDeclaration>| {
            members
                .into_iter()
                .map(|member| {
                    let key = member
                        .descriptor
                        .split_once("->")
                        .map(|(_, key)| key.to_owned())
                        .expect("member descriptors name their class before `->`");
                    let member = Member {
                        flags: member.flags,
                        node: Node::new(member.declaration),
                    };
                    (key, member)
                })
                .collect()
        };
        Self {
            jar,
            supertypes: class
                .superclass
                .into_iter()
                .chain(class.interfaces)
                .collect(),
            header: Node::new(class.header),
            fields: members(class.fields),
            methods: members(class.methods),
        }
    }

    /// What a reached class brings with it: its initializer, and what code outside
    /// the bundle can reach through an instance, its virtual methods and instance fields.
    pub(super) fn implicit(&self) -> impl Iterator<Item = &String> {
        let methods = self.methods.iter().filter(|(key, method)| {
            key.starts_with("<clinit>(") || MethodKind::of(method.flags) == MethodKind::Virtual
        });
        let fields = self
            .fields
            .iter()
            .filter(|(_, field)| !field.flags.contains(AccessFlags::STATIC));
        methods.chain(fields).map(|(key, _)| key)
    }

    pub(super) fn member(&self, key: &str) -> Option<(&String, &Member)> {
        self.methods
            .get_key_value(key)
            .or_else(|| self.fields.get_key_value(key))
    }
}

impl Node {
    fn new(declaration: Declaration) -> Self {
        Self {
            hash: Sha256::digest(declaration.text.as_bytes()).into(),
            references: declaration.references,
        }
    }
}

/// A patch jar's `classes.dex`, and its other files apart from JVM classes and
/// the declaration index, which [`crate::bundle::declarations`] reads.
fn read_jar(name: &str, path: &Path) -> Result<(Vec<u8>, HashedFiles)> {
    let jar_error = |source| BundleError::Jar {
        name: name.to_owned(),
        source,
    };
    let mut archive = ZipArchive::new(std::fs::File::open(path)?).map_err(jar_error)?;
    let mut dex = None;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(jar_error)?;
        let entry_name = entry.name().to_owned();
        let class = Path::new(&entry_name)
            .extension()
            .is_some_and(|extension| extension == "class");
        if entry.is_dir() || class || entry_name == PATCH_INDEX {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if entry_name == "classes.dex" {
            dex = Some(bytes);
        } else {
            files.push((entry_name, Sha256::digest(&bytes).into()));
        }
    }
    files.sort();
    let dex = dex.ok_or_else(|| BundleError::IncompleteJar {
        name: name.to_owned(),
        missing: JarPart::Dex,
    })?;
    Ok((dex, files))
}
