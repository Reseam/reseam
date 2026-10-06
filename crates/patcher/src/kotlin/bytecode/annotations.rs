// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::fields::field_named;
use super::logged;
use crate::kotlin::convert::import_value;
use crate::kotlin::handles::{method_ref, with_class_mut, with_method_mut};
use crate::kotlin::link::{link_descriptors, value_types};
use crate::kotlin::types::AnnotationItem;
use boltffi::export;
use reseam_apk::reseam_dex::{
    AnnotationElement as DexAnnotationElement, AnnotationItem as DexAnnotationItem,
    AnnotationVisibility, DexFile,
};

#[export]
pub fn add_class_annotation(c: u32, annotation: AnnotationItem) {
    link_descriptors(annotation_types(&annotation));
    with_class_mut(c, |dex, loc| {
        let annotation = logged(
            "convert annotation",
            annotation_item(&annotation, dex, loc.dex_idx),
        )?;
        let class = logged("add class annotation", dex.class_mut(loc.class_idx))?;
        logged(
            "read annotations",
            class
                .annotations
                .get_or_insert_with(Default::default)
                .resolve_mut(),
        )?
        .class
        .push(annotation);
        Ok(Some(()))
    });
}

#[export]
pub fn add_method_annotation(m: u32, annotation: AnnotationItem) {
    link_descriptors(annotation_types(&annotation));
    with_method_mut(m, |dex, loc| {
        let Some(method_idx) = method_ref(dex, loc).map(|method| method.method) else {
            return Err(format!("method handle {m} has no declaration"));
        };
        let annotation = logged(
            "convert annotation",
            annotation_item(&annotation, dex, loc.dex_idx),
        )?;
        let directory = logged("add method annotation", dex.class_mut(loc.class_idx))?
            .annotations
            .get_or_insert_with(Default::default);
        let directory = logged("read annotations", directory.resolve_mut())?;
        match directory
            .methods
            .iter_mut()
            .find(|(idx, _)| *idx == method_idx)
        {
            Some((_, list)) => list.push(annotation),
            None => directory.methods.push((method_idx, vec![annotation])),
        }
        Ok(Some(()))
    });
}

#[export]
pub fn add_field_annotation(c: u32, field_name: String, annotation: AnnotationItem) {
    link_descriptors(annotation_types(&annotation));
    with_class_mut(c, |dex, loc| {
        let Some(field) = field_named(dex, loc.class_idx, &field_name) else {
            return Err(format!("class handle {c} has no field {field_name}"));
        };
        let annotation = logged(
            "convert annotation",
            annotation_item(&annotation, dex, loc.dex_idx),
        )?;
        let directory = logged("add field annotation", dex.class_mut(loc.class_idx))?
            .annotations
            .get_or_insert_with(Default::default);
        let directory = logged("read annotations", directory.resolve_mut())?;
        match directory.fields.iter_mut().find(|(idx, _)| *idx == field) {
            Some((_, list)) => list.push(annotation),
            None => directory.fields.push((field, vec![annotation])),
        }
        Ok(Some(()))
    });
}

fn annotation_types(item: &AnnotationItem) -> impl Iterator<Item = &str> {
    std::iter::once(item.annotation_type.as_str()).chain(
        item.elements
            .iter()
            .flat_map(|element| value_types(&element.value)),
    )
}

fn annotation_item(
    item: &AnnotationItem,
    dex: &mut DexFile,
    dex_index: usize,
) -> reseam_apk::reseam_dex::Result<DexAnnotationItem> {
    Ok(DexAnnotationItem {
        visibility: match item.visibility {
            1 => AnnotationVisibility::Runtime,
            2 => AnnotationVisibility::System,
            0 => AnnotationVisibility::Build,
            value => {
                return Err(reseam_apk::reseam_dex::DexError::Invalid {
                    section: "annotation visibility",
                    reason: format!("unknown value {value}"),
                });
            }
        },
        type_: dex.intern_type(&item.annotation_type)?,
        elements: item
            .elements
            .iter()
            .map(|element| {
                Ok(DexAnnotationElement {
                    name: dex.intern_string(&element.name),
                    value: import_value(dex, Some(dex_index), &element.value)?,
                })
            })
            .collect::<reseam_apk::reseam_dex::Result<_>>()?,
    })
}
