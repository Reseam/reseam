// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Bodies, RefSink, class, class_header, id_entry, raw_class};
use crate::DexFile;
use crate::error::Result;
use crate::model::Pool;
use crate::model::class::ClassDef;

/// Records the first reference `check` rejects.
pub(crate) struct ReferenceValidator<'a> {
    check: &'a dyn Fn(Pool, u32) -> Result<()>,
    error: Option<crate::DexError>,
}

impl<'a> ReferenceValidator<'a> {
    pub(crate) fn new(check: &'a dyn Fn(Pool, u32) -> Result<()>) -> Self {
        Self { check, error: None }
    }
    pub(crate) fn finish(self) -> Result<()> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl RefSink for ReferenceValidator<'_> {
    fn add(&mut self, pool: Pool, index: u32) {
        if self.error.is_none()
            && let Err(error) = (self.check)(pool, index)
        {
            self.error = Some(error);
        }
    }
}

pub(crate) fn check_index(dex: &DexFile, pool: Pool, index: u32) -> Result<()> {
    let count = dex.pool_len(pool);
    if index as usize >= count {
        return Err(crate::error::invalid(
            "pool reference",
            format!("{pool:?} index {index} exceeds {count} entries"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_pools(dex: &DexFile) -> Result<()> {
    let in_pools = |pool, index| check_index(dex, pool, index);
    let mut validator = ReferenceValidator::new(&in_pools);
    for pool in Pool::ALL {
        for idx in 0..dex.pool_len(pool) as u32 {
            id_entry(&mut validator, dex, pool, idx)?;
        }
    }
    for index in 0..dex.classes.len() {
        class_header(
            &mut validator,
            dex.classes.header(index),
            &dex.classes.interfaces(index),
        );
    }
    validator.finish()
}

pub(crate) fn validate_class(dex: &DexFile, item: &ClassDef) -> Result<()> {
    let in_pools = |pool, index| check_index(dex, pool, index);
    let mut validator = ReferenceValidator::new(&in_pools);
    class(&mut validator, item, dex, Bodies::Visit)?;
    validator.finish()
}

pub(crate) fn validate_skeleton(dex: &DexFile, skeleton: &crate::ClassSkeleton) -> Result<()> {
    let in_pools = |pool, index| check_index(dex, pool, index);
    let mut validator = ReferenceValidator::new(&in_pools);
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

/// Checks that the part's pools hold every class-level reference of its
/// classes. Method bodies are not walked: writing a part remaps each of their
/// references through its pool tables, which reject any reference outside it.
pub(crate) fn validate_part(dex: &DexFile, part: &crate::DexPart) -> Result<()> {
    let in_part = |pool, index| {
        check_index(dex, pool, index)?;
        if !part.pools.contains(pool, index) {
            return Err(crate::error::invalid(
                "DEX part",
                format!("{pool:?} index {index} is absent from the selected pools"),
            ));
        }
        Ok(())
    };
    let mut validator = ReferenceValidator::new(&in_part);
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
