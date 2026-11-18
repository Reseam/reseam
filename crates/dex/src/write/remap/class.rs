// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::Remap;
use crate::model::class::{ClassDef, ClassHeader};
use crate::model::{FieldIdx, MethodIdx, Pool};
use crate::references;

impl Remap {
    /// Rewrites every index `class` holds: its header, members, bodies,
    /// annotations and static values.
    pub(crate) fn remap_class(&self, class: &mut ClassDef) -> crate::Result<()> {
        self.check(|refs| {
            references::class_header(refs, ClassHeader::of(class), &class.interfaces);
            Ok(())
        })?;
        class.class_type = self.remap_type(class.class_type);
        class.superclass = class.superclass.map(|ty| self.remap_type(ty));
        for interface in &mut class.interfaces {
            *interface = self.remap_type(*interface);
        }
        class.source_file = class.source_file.map(|name| self.remap_string(name));
        if let Some(annotations) = &mut class.annotations {
            self.remap_annotations_dir(annotations.resolve_mut()?)?;
        }
        if let Some(data) = &mut class.class_data {
            for field in data
                .static_fields
                .iter_mut()
                .chain(&mut data.instance_fields)
            {
                field.field = self.member_field(field.field)?;
            }
            for method in data
                .direct_methods
                .iter_mut()
                .chain(&mut data.virtual_methods)
            {
                method.method = MethodIdx(self.index(Pool::Method, method.method.0)?);
                if let Some(code) = method.code_mut() {
                    self.remap_code(code)?;
                }
            }
        }
        class.static_values = std::mem::take(&mut class.static_values)
            .into_iter()
            .map(|(field, mut value)| {
                self.remap_encoded_value(&mut value)?;
                Ok((self.member_field(field)?, value))
            })
            .collect::<crate::Result<_>>()?;
        Ok(())
    }

    fn member_field(&self, field: FieldIdx) -> crate::Result<FieldIdx> {
        self.index(Pool::Field, field.0).map(FieldIdx)
    }
}
