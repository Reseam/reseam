// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::logged;
use crate::kotlin::convert::import_value;
use crate::kotlin::handles::with_class_mut;
use crate::kotlin::link::{link_descriptors, value_types};
use crate::kotlin::types::{EncodedVal, NewField};
use boltffi::export;
use reseam_apk::reseam_dex::{AccessFlags, DexFile, EncodedField, EncodedValue, FieldIdx};

#[export]
pub fn add_field(c: u32, field: NewField) {
    link_descriptors([field.field_type.as_str()]);
    with_class_mut(c, |dex, loc| {
        let class_desc = dex
            .type_descriptor(dex.class_header(loc.class_idx).class_type)
            .into_owned();
        let field_idx = logged(
            "intern field",
            dex.intern_field(&class_desc, &field.name, &field.field_type),
        )?;
        let flags = AccessFlags::from_bits_retain(field.access_flags);
        let initial_value = logged(
            "convert initial value",
            field
                .initial_value
                .as_ref()
                .map(|v| import_value(dex, Some(loc.dex_idx), v))
                .transpose(),
        )?;
        let encoded = EncodedField {
            field: field_idx,
            access_flags: flags,
        };
        if !flags.contains(AccessFlags::STATIC) {
            logged("add field", dex.class_mut(loc.class_idx))?.add_instance_field(encoded);
            return Ok(Some(()));
        }
        let class = logged("add field", dex.class_mut(loc.class_idx))?;
        class.add_static_field(encoded);
        if let Some(value) = initial_value {
            let slot = class
                .class_data
                .as_ref()
                .map_or(0, |d| d.static_fields.len())
                - 1;
            if set_static_value(dex, loc.class_idx, slot, value)?.is_none() {
                return Ok(None);
            }
        }
        Ok(Some(()))
    });
}

fn set_static_value(
    dex: &mut DexFile,
    class_idx: usize,
    slot: usize,
    value: EncodedValue,
) -> Result<Option<()>, String> {
    let Some(class) = dex.resident_class(class_idx) else {
        return Ok(None);
    };
    let Some(fields) = class.class_data.as_ref().map(|data| &data.static_fields) else {
        return Ok(None);
    };
    let Some(field) = fields.get(slot).map(|field| field.field) else {
        return Ok(None);
    };
    logged("set static field value", dex.class_mut(class_idx))?
        .static_values
        .insert(field, value);
    Ok(Some(()))
}

pub(super) fn field_named(dex: &DexFile, class_idx: usize, name: &str) -> Option<FieldIdx> {
    let data = dex.resident_class(class_idx)?.class_data.as_ref()?;
    data.static_fields
        .iter()
        .chain(&data.instance_fields)
        .map(|f| f.field)
        .find(|&field| dex.string(dex.field_id(field).name) == name)
}

#[export]
pub fn remove_field(c: u32, name: String) {
    with_class_mut(c, |dex, loc| {
        let Some(field) = field_named(dex, loc.class_idx, &name) else {
            return Ok(None);
        };
        let class = logged("remove field", dex.class_mut(loc.class_idx))?;
        let Some(data) = class.class_data.as_mut() else {
            return Ok(None);
        };
        if let Some(slot) = data
            .static_fields
            .iter()
            .position(|entry| entry.field == field)
        {
            data.static_fields.remove(slot);
            class.static_values.remove(&field);
        } else {
            data.instance_fields.retain(|entry| entry.field != field);
        }
        if let Some(metadata) = class.annotations.as_mut() {
            let annotations = logged("read annotations", metadata.resolve_mut())?;
            annotations.fields.retain(|(id, _)| *id != field);
        }
        Ok(Some(()))
    });
}

#[export]
pub fn set_field_access_flags(c: u32, field_name: String, flags: u32) {
    with_class_mut(c, |dex, loc| {
        let field = field_named(dex, loc.class_idx, &field_name)
            .ok_or_else(|| format!("class handle {c} has no field {field_name}"))?;
        let flags = AccessFlags::from_bits_retain(flags);
        let class = logged("set field access flags", dex.class_mut(loc.class_idx))?;
        let data = class
            .class_data
            .as_mut()
            .expect("field was found in class data");
        if let Some(index) = data
            .static_fields
            .iter()
            .position(|entry| entry.field == field)
        {
            data.static_fields[index].access_flags = flags;
            if !flags.contains(AccessFlags::STATIC) {
                data.instance_fields.push(data.static_fields.remove(index));
                class.static_values.remove(&field);
            }
        } else {
            let index = data
                .instance_fields
                .iter()
                .position(|entry| entry.field == field)
                .expect("field was found in one member group");
            data.instance_fields[index].access_flags = flags;
            if flags.contains(AccessFlags::STATIC) {
                data.static_fields.push(data.instance_fields.remove(index));
            }
        }
        Ok(Some(()))
    });
}

#[export]
pub fn set_static_field_value(c: u32, field_name: String, value: EncodedVal) {
    link_descriptors(value_types(&value));
    with_class_mut(c, |dex, loc| {
        let Some(slot) = dex
            .resident_class(loc.class_idx)
            .and_then(|class| class.class_data.as_ref())
            .and_then(|data| {
                data.static_fields
                    .iter()
                    .position(|f| dex.string(dex.field_id(f.field).name) == field_name)
            })
        else {
            return Err(format!("class handle {c} has no static field {field_name}"));
        };
        let value = logged(
            "convert static value",
            import_value(dex, Some(loc.dex_idx), &value),
        )?;
        set_static_value(dex, loc.class_idx, slot, value)
    });
}
