// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod reader;
mod writer;

use std::ops::Range;

use reseam_storage::Bytes;

use super::config::{config_for_qualifiers, same_config};
use super::{ResType, TypeSpec, first_found};
use crate::error::Result;
use crate::string_pool::StringPool;

const HEADER_LEN: usize = 288;
pub(super) const NAME_UNITS: usize = 128;

#[derive(Debug, Clone)]
pub struct ResPackage {
    pub(super) id: u32,
    name: [u16; NAME_UNITS],
    pub(super) type_strings: StringPool,
    pub(super) key_strings: StringPool,
    pub(super) last_public_type: u32,
    pub(super) last_public_key: u32,
    pub(super) type_id_offset: u32,
    pub(super) type_specs: Vec<TypeSpec>,
    pub(super) types: Vec<ResType>,
    pub(super) data: Bytes,
    chunk: Range<usize>,
    header_size: usize,
    pub(super) chunks: Vec<PackageChunk>,
}

#[derive(Debug, Clone)]
pub(super) enum PackageChunk {
    TypeStrings,
    KeyStrings,
    Spec(usize),
    Type(usize),
    Raw(Range<usize>),
}

impl ResPackage {
    pub fn new(id: u32, name: &str, type_strings: StringPool, key_strings: StringPool) -> Self {
        Self {
            id,
            name: Self::encode_name(name),
            type_strings,
            key_strings,
            last_public_type: 0,
            last_public_key: 0,
            type_id_offset: 0,
            type_specs: Vec::new(),
            types: Vec::new(),
            data: Bytes::default(),
            chunk: 0..0,
            header_size: HEADER_LEN,
            chunks: vec![PackageChunk::TypeStrings, PackageChunk::KeyStrings],
        }
    }

    pub(super) fn resource_type_id(&self, local: u8) -> Result<u8> {
        u32::from(local)
            .checked_add(self.type_id_offset)
            .and_then(|id| u8::try_from(id).ok())
            .ok_or_else(|| {
                crate::error::invalid("resource type", "type ID plus package offset exceeds 255")
            })
    }

    pub(super) fn local_type_id(&self, effective: u8) -> Option<u8> {
        u32::from(effective)
            .checked_sub(self.type_id_offset)
            .and_then(|id| u8::try_from(id).ok())
    }

    /// Builds an ID from a local type ID and entry index, applying this package's
    /// type offset. IDs outside Android's package, type or entry ranges are errors.
    pub fn resource_id(&self, local: u8, index: usize) -> Result<u32> {
        let package = u8::try_from(self.id)
            .map_err(|_| crate::error::invalid("resource package", "package ID exceeds 255"))?;
        let index = u16::try_from(index)
            .map_err(|_| crate::error::invalid("resource entry", "entry index exceeds 65535"))?;
        Ok(super::res_id(
            u32::from(package),
            self.resource_type_id(local)?,
            usize::from(index),
        ))
    }

    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn types(&self) -> &[ResType] {
        &self.types
    }
    pub fn type_specs(&self) -> &[TypeSpec] {
        &self.type_specs
    }
    pub fn type_strings(&self) -> &StringPool {
        &self.type_strings
    }
    pub fn key_strings(&self) -> &StringPool {
        &self.key_strings
    }

    /// Adds a configuration chunk after the existing chunks, retaining their order.
    pub fn add_type(&mut self, res_type: ResType) {
        self.chunks.push(PackageChunk::Type(self.types.len()));
        self.types.push(res_type);
    }

    /// Adds a type specification after the existing chunks.
    pub fn add_type_spec(&mut self, spec: TypeSpec) {
        self.chunks.push(PackageChunk::Spec(self.type_specs.len()));
        self.type_specs.push(spec);
    }

    /// Decodes the NUL-terminated package name, replacing unpaired UTF-16 surrogates.
    pub fn name(&self) -> String {
        let end = self
            .name
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(NAME_UNITS);
        String::from_utf16_lossy(&self.name[..end])
    }

    pub(crate) fn set_name(&mut self, name: &str) {
        self.name = Self::encode_name(name);
    }

    fn encode_name(name: &str) -> [u16; NAME_UNITS] {
        let mut units = [0; NAME_UNITS];
        for (slot, unit) in units.iter_mut().zip(name.encode_utf16()) {
            *slot = unit;
        }
        units
    }

    pub(crate) fn ensure_type(&mut self, type_name: &str) -> Result<Option<u8>> {
        if let Some(index) = self.type_strings.find(type_name)? {
            return Ok(u8::try_from(index + 1).ok());
        }
        if self.type_strings.len() >= u8::MAX as usize {
            return Ok(None);
        }
        let type_id = (self.type_strings.len() + 1) as u8;
        self.resource_type_id(type_id)?;
        self.type_strings.push(type_name);
        self.add_type_spec(TypeSpec::new(type_id, Vec::new()));
        let config = config_for_qualifiers("", self.config_len())?;
        self.add_type(ResType::new(type_id, config));
        Ok(Some(type_id))
    }

    pub(crate) fn config_len(&self) -> usize {
        self.types.first().map_or(4, ResType::config_len).max(4)
    }

    pub(crate) fn entry_count(&self, type_id: u8) -> usize {
        self.types
            .iter()
            .filter(|res_type| res_type.id == type_id)
            .map(ResType::len)
            .chain(
                self.type_specs
                    .iter()
                    .filter(|spec| spec.id == type_id)
                    .map(TypeSpec::len),
            )
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn entry_index(&self, type_id: u8, key: u32) -> Result<Option<usize>> {
        first_found(
            self.types
                .iter()
                .filter(|res_type| res_type.id == type_id)
                .flat_map(|res_type| {
                    (0..res_type.len()).map(|i| {
                        res_type
                            .entry_key(i)
                            .map(|entry_key| (entry_key == Some(key)).then_some(i))
                    })
                }),
        )
    }

    pub(crate) fn config_type(&mut self, type_id: u8, config: Vec<u8>) -> &mut ResType {
        let existing = self
            .types
            .iter()
            .position(|res_type| res_type.id == type_id && same_config(res_type.config(), &config));
        let index = existing.unwrap_or_else(|| {
            self.add_type(ResType::new(type_id, config));
            self.types.len() - 1
        });
        &mut self.types[index]
    }

    pub(crate) fn grow_type(&mut self, type_id: u8, len: usize) {
        for spec in self.type_specs.iter_mut().filter(|spec| spec.id == type_id) {
            while spec.len() < len {
                spec.push(0);
            }
        }
        for res_type in self.types.iter_mut().filter(|t| t.id == type_id) {
            res_type.pad_to(len);
        }
    }
}
