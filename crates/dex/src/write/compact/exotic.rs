// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use crate::types::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::types::class::ClassDef;
use crate::types::encoded_value::EncodedValue;
use crate::types::instruction::Instruction;
use crate::types::method_handle::{CallSiteIdx, MethodHandleIdx};

pub(super) fn remap_exotic_refs(
    class: &mut ClassDef,
    cs_remap: &HashMap<u32, u32>,
    mh_remap: &HashMap<u32, u32>,
) {
    if let Some(data) = &mut class.class_data {
        for method in data
            .direct_methods
            .iter_mut()
            .chain(data.virtual_methods.iter_mut())
        {
            if let Some(code) = &mut method.code {
                for insn in &mut code.instructions {
                    match insn {
                        Instruction::InvokeCustom { call_site, .. }
                        | Instruction::InvokeCustomRange { call_site, .. } => {
                            if let Some(&new_cs) = cs_remap.get(&call_site.0) {
                                *call_site = CallSiteIdx(new_cs);
                            }
                        }
                        Instruction::ConstMethodHandle { method_handle, .. } => {
                            if let Some(&new_mh) = mh_remap.get(&method_handle.0) {
                                *method_handle = MethodHandleIdx(new_mh);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    for static_value in &mut class.static_values {
        remap_method_handle_in_encoded_value(static_value, mh_remap);
    }

    if let Some(annotations) = &mut class.annotations {
        remap_method_handle_in_annotations_dir(annotations, mh_remap);
    }
}

pub(super) fn remap_all_exotic_indices(
    dex: &mut crate::file::DexFile,
    cs_remap: &[u32],
    mh_remap: &[u32],
) -> crate::error::Result<()> {
    for class_idx in 0..dex.classes.len() {
        let class = dex.class_mut(class_idx)?;
        if let Some(data) = &mut class.class_data {
            for method in data
                .direct_methods
                .iter_mut()
                .chain(data.virtual_methods.iter_mut())
            {
                if let Some(code) = &mut method.code {
                    for insn in &mut code.instructions {
                        match insn {
                            Instruction::InvokeCustom { call_site, .. }
                            | Instruction::InvokeCustomRange { call_site, .. } => {
                                if let Some(&new) = cs_remap.get(call_site.0 as usize) {
                                    if new != u32::MAX {
                                        call_site.0 = new;
                                    }
                                }
                            }
                            Instruction::ConstMethodHandle { method_handle, .. } => {
                                if let Some(&new) = mh_remap.get(method_handle.0 as usize) {
                                    if new != u32::MAX {
                                        method_handle.0 = new;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        for static_value in &mut class.static_values {
            remap_method_handle_index_in_encoded_value(static_value, mh_remap);
        }
        if let Some(annotations) = &mut class.annotations {
            remap_method_handle_index_in_annotations_dir(annotations, mh_remap);
        }
    }
    Ok(())
}

fn remap_method_handle_in_encoded_value(value: &mut EncodedValue, mh_remap: &HashMap<u32, u32>) {
    match value {
        EncodedValue::MethodHandle(idx) => {
            if let Some(&new_mh) = mh_remap.get(&idx.0) {
                *idx = MethodHandleIdx(new_mh);
            }
        }
        EncodedValue::Array(items) => {
            for item in items {
                remap_method_handle_in_encoded_value(item, mh_remap);
            }
        }
        EncodedValue::Annotation(annotation) => {
            for elem in &mut annotation.elements {
                remap_method_handle_in_encoded_value(&mut elem.value, mh_remap);
            }
        }
        _ => {}
    }
}

fn remap_method_handle_in_annotations_dir(
    dir: &mut AnnotationsDirectory,
    mh_remap: &HashMap<u32, u32>,
) {
    for item in &mut dir.class_annotations {
        remap_method_handle_in_annotation_item(item, mh_remap);
    }
    for (_, items) in &mut dir.field_annotations {
        for item in items {
            remap_method_handle_in_annotation_item(item, mh_remap);
        }
    }
    for (_, items) in &mut dir.method_annotations {
        for item in items {
            remap_method_handle_in_annotation_item(item, mh_remap);
        }
    }
    for (_, param_items) in &mut dir.parameter_annotations {
        for items in param_items {
            for item in items {
                remap_method_handle_in_annotation_item(item, mh_remap);
            }
        }
    }
}

fn remap_method_handle_in_annotation_item(item: &mut AnnotationItem, mh_remap: &HashMap<u32, u32>) {
    for elem in &mut item.elements {
        remap_method_handle_in_encoded_value(&mut elem.value, mh_remap);
    }
}

fn remap_method_handle_index_in_encoded_value(value: &mut EncodedValue, mh_remap: &[u32]) {
    match value {
        EncodedValue::MethodHandle(idx) => {
            if let Some(&new) = mh_remap.get(idx.0 as usize) {
                if new != u32::MAX {
                    idx.0 = new;
                }
            }
        }
        EncodedValue::Array(items) => {
            for item in items {
                remap_method_handle_index_in_encoded_value(item, mh_remap);
            }
        }
        EncodedValue::Annotation(annotation) => {
            for elem in &mut annotation.elements {
                remap_method_handle_index_in_encoded_value(&mut elem.value, mh_remap);
            }
        }
        _ => {}
    }
}

fn remap_method_handle_index_in_annotations_dir(dir: &mut AnnotationsDirectory, mh_remap: &[u32]) {
    for item in &mut dir.class_annotations {
        for elem in &mut item.elements {
            remap_method_handle_index_in_encoded_value(&mut elem.value, mh_remap);
        }
    }
    for (_, items) in &mut dir.field_annotations {
        for item in items {
            for elem in &mut item.elements {
                remap_method_handle_index_in_encoded_value(&mut elem.value, mh_remap);
            }
        }
    }
    for (_, items) in &mut dir.method_annotations {
        for item in items {
            for elem in &mut item.elements {
                remap_method_handle_index_in_encoded_value(&mut elem.value, mh_remap);
            }
        }
    }
    for (_, param_items) in &mut dir.parameter_annotations {
        for items in param_items {
            for item in items {
                for elem in &mut item.elements {
                    remap_method_handle_index_in_encoded_value(&mut elem.value, mh_remap);
                }
            }
        }
    }
}
