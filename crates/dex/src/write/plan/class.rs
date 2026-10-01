// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    AnnotationsDirectory, ClassHeader, Cow, EncodedValue, Result, TypeList, WriteClass, WritePlan,
    read_annotations_directory,
};

impl WritePlan<'_> {
    pub(crate) fn class_header(&self, k: usize) -> ClassHeader {
        let header = match &self.classes[k] {
            WriteClass::Resident(class) => ClassHeader::of(class),
            WriteClass::Raw(raw) => raw.header(),
        };
        ClassHeader {
            class_type: self.map_type(header.class_type),
            access_flags: header.access_flags,
            superclass: header.superclass.map(|ty| self.map_type(ty)),
            source_file: header.source_file.map(|name| self.map_string(name)),
        }
    }

    pub(crate) fn class_interfaces(&self, k: usize) -> TypeList {
        let interfaces = match &self.classes[k] {
            WriteClass::Resident(class) => class.interfaces.clone(),
            WriteClass::Raw(raw) if raw.interfaces_off != 0 => {
                crate::file::read_type_list(self.raw_bytes(), raw.interfaces_off as usize)
            }
            WriteClass::Raw(_) => TypeList::new(),
        };
        interfaces.iter().map(|ty| self.map_type(*ty)).collect()
    }

    pub(crate) fn class_annotations(
        &self,
        k: usize,
    ) -> Result<Option<Cow<'_, AnnotationsDirectory>>> {
        if self.options.annotations == super::super::MetadataPolicy::Omit {
            return Ok(None);
        }
        match &self.classes[k] {
            WriteClass::Resident(c) => c
                .annotations
                .as_deref()
                .filter(|metadata| {
                    self.options
                        .annotations
                        .keeps(metadata, self.dex.raw.as_ref())
                })
                .map(|metadata| {
                    let dir = metadata.read()?;
                    Ok(match self.remap() {
                        Some(remap) => {
                            let mut dir = dir.into_owned();
                            remap.remap_annotations_dir(&mut dir)?;
                            Cow::Owned(dir)
                        }
                        None => dir,
                    })
                })
                .transpose(),
            WriteClass::Raw(raw) => {
                if raw.annotations_off == 0
                    || self.options.annotations != super::super::MetadataPolicy::Preserve
                {
                    return Ok(None);
                }
                let mut dir = read_annotations_directory(
                    self.raw_bytes(),
                    raw.annotations_off,
                    self.dex.parse_options,
                )?;
                if let Some(remap) = self.remap() {
                    remap.remap_annotations_dir(&mut dir)?;
                }
                Ok(Some(Cow::Owned(dir)))
            }
        }
    }

    pub(crate) fn class_static_values(&self, k: usize) -> Result<Vec<EncodedValue>> {
        let source = self.class_order[k];
        let values = self.dex.class_static_values(source)?;
        let mut fields = self
            .dex
            .decode_class_fields(source)?
            .map_or_else(Vec::new, |fields| fields.0);
        fields.sort_by_key(|field| {
            self.remap()
                .map_or(field.field, |remap| remap.remap_field(field.field))
        });
        let end = fields
            .iter()
            .rposition(|field| {
                values
                    .get(&field.field)
                    .is_some_and(|value| !super::super::is_default_value(value))
            })
            .map_or(0, |i| i + 1);
        fields[..end]
            .iter()
            .map(|field| {
                let mut value = values
                    .get(&field.field)
                    .cloned()
                    .unwrap_or_else(|| self.dex.default_field_value(field.field));
                if let Some(remap) = self.remap() {
                    remap.remap_encoded_value(&mut value)?;
                }
                Ok(value)
            })
            .collect()
    }
}
