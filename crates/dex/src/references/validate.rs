// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Bodies, RefSink, call_site, class, raw_class};
use crate::DexFile;
use crate::error::Result;
use crate::read::type_list::read_type_list;
use crate::types::Pool;
use crate::types::class::ClassDef;

pub(crate) struct ReferenceValidator<F> {
    check: F,
    error: Option<crate::DexError>,
}

impl<F: Fn(Pool, u32) -> Result<()>> ReferenceValidator<F> {
    pub(crate) fn new(check: F) -> Self {
        Self { check, error: None }
    }
    pub(crate) fn finish(self) -> Result<()> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl<F: Fn(Pool, u32) -> Result<()>> RefSink for ReferenceValidator<F> {
    fn add(&mut self, pool: Pool, index: u32) {
        if self.error.is_none()
            && let Err(error) = (self.check)(pool, index)
        {
            self.error = Some(error);
        }
    }
}

pub(crate) fn check_index(dex: &DexFile, pool: Pool, index: u32) -> Result<()> {
    let count = pool_len(dex, pool);
    if index as usize >= count {
        return Err(crate::error::invalid(
            "pool reference",
            format!("{pool:?} index {index} exceeds {count} entries"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_pools(dex: &DexFile) -> Result<()> {
    let mut validator = ReferenceValidator::new(|pool, index| check_index(dex, pool, index));
    for descriptor in dex.types.iter() {
        validator.add(Pool::String, descriptor.0);
    }
    for proto in dex.prototypes.iter() {
        validator.add(Pool::String, proto.shorty.0);
        validator.add(Pool::Type, proto.return_type.0);
        for parameter in proto.parameters {
            validator.add(Pool::Type, parameter.0);
        }
    }
    for field in dex.fields.iter() {
        validator.add(Pool::Type, field.class.0);
        validator.add(Pool::Type, field.type_.0);
        validator.add(Pool::String, field.name.0);
    }
    for method in dex.methods.iter() {
        validator.add(Pool::Type, method.class.0);
        validator.add(Pool::Proto, method.proto.0);
        validator.add(Pool::String, method.name.0);
    }
    for handle in dex.method_handles.iter() {
        match handle?.member {
            crate::types::method_handle::MethodHandleMember::Field(field) => {
                validator.add(Pool::Field, field.0);
            }
            crate::types::method_handle::MethodHandleMember::Method(method) => {
                validator.add(Pool::Method, method.0);
            }
        }
    }
    for site in dex.call_sites.iter() {
        call_site(&mut validator, site?.as_ref());
    }
    for index in 0..dex.classes.len() {
        let header = dex.classes().header(index);
        validator.add(Pool::Type, header.class_type.0);
        if let Some(superclass) = header.superclass {
            validator.add(Pool::Type, superclass.0);
        }
        if let Some(source) = header.source_file {
            validator.add(Pool::String, source.0);
        }
        if let Some(class) = dex.classes.resident(index) {
            for interface in &class.interfaces {
                validator.add(Pool::Type, interface.0);
            }
        } else if let Some(raw) = dex.classes.raw_def(index) {
            let buffer = dex.raw_buffer().ok_or_else(|| {
                crate::error::invalid("DEX source", "deferred classes require the original buffer")
            })?;
            if raw.interfaces_off != 0 {
                for interface in read_type_list(buffer, raw.interfaces_off as usize) {
                    validator.add(Pool::Type, interface.0);
                }
            }
        }
    }
    validator.finish()
}

pub(crate) fn validate_class(dex: &DexFile, item: &ClassDef) -> Result<()> {
    let mut validator = ReferenceValidator::new(|pool, index| check_index(dex, pool, index));
    class(&mut validator, item, dex, Bodies::Visit)?;
    validator.finish()
}

pub(crate) fn validate_skeleton(dex: &DexFile, skeleton: &crate::ClassSkeleton) -> Result<()> {
    let mut validator = ReferenceValidator::new(|pool, index| check_index(dex, pool, index));
    for field in skeleton
        .static_fields
        .iter()
        .chain(&skeleton.instance_fields)
    {
        validator.add(Pool::Field, field.field.0);
    }
    for method in skeleton
        .direct_methods
        .iter()
        .chain(&skeleton.virtual_methods)
    {
        validator.add(Pool::Method, method.method.0);
    }
    validator.finish()
}

pub(crate) fn pool_len(dex: &DexFile, pool: Pool) -> usize {
    match pool {
        Pool::String => dex.strings.len(),
        Pool::Type => dex.types.len(),
        Pool::Proto => dex.prototypes.len(),
        Pool::Field => dex.fields.len(),
        Pool::Method => dex.methods.len(),
        Pool::CallSite => dex.call_sites.len(),
        Pool::MethodHandle => dex.method_handles.len(),
    }
}

/// Checks that the part's pools hold every class-level reference of its
/// classes. Method bodies are not walked: writing a part remaps each of their
/// references through its pool tables, which reject any reference outside it.
pub(crate) fn validate_part(dex: &DexFile, part: &crate::DexPart) -> Result<()> {
    let mut validator = ReferenceValidator::new(|pool, index| {
        check_index(dex, pool, index)?;
        if !part.pools.contains(pool, index) {
            return Err(crate::error::invalid(
                "DEX part",
                format!("{pool:?} index {index} is absent from the selected pools"),
            ));
        }
        Ok(())
    });
    for &index in &part.classes {
        if index >= dex.classes.len() {
            return Err(crate::error::invalid(
                "DEX part",
                "class index exceeds the source class table",
            ));
        }
        if let Some(item) = dex.classes.resident(index) {
            class(&mut validator, item, dex, Bodies::Skip)?;
        } else if let Some(raw) = dex.classes.raw_def(index) {
            raw_class(
                &mut validator,
                dex.raw_buffer().ok_or_else(|| {
                    crate::error::invalid(
                        "DEX source",
                        "deferred classes require the original buffer",
                    )
                })?,
                &raw,
                dex.parse_options,
                dex.write_options(),
                Bodies::Skip,
            )?;
        }
    }
    validator.finish()
}
