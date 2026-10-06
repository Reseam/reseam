// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::encoded_value::read_encoded_annotation_with_opts;
use crate::error::{Result, invalid_annotation_visibility};
use crate::read::{read_u8, read_u32};
use crate::types::annotation::{
    AnnotationElement, AnnotationItem, AnnotationVisibility, AnnotationsDirectory,
};
use crate::types::header::ParseOptions;
use crate::types::{FieldIdx, MethodIdx};

pub fn read_annotations_directory(
    buf: &[u8],
    off: u32,
    opts: ParseOptions,
) -> Result<AnnotationsDirectory> {
    let base = off as usize;
    let class_annotations_off = read_u32(buf, base)?;
    let fields_size = read_u32(buf, base + 4)? as usize;
    let methods_size = read_u32(buf, base + 8)? as usize;
    let params_size = read_u32(buf, base + 12)? as usize;

    let mut pos = base + 16;

    let mut field_annotations =
        Vec::with_capacity(fields_size.min(buf.len().saturating_sub(pos) / 8));
    for _ in 0..fields_size {
        let field_idx = FieldIdx(read_u32(buf, pos)?);
        let ann_off = read_u32(buf, pos + 4)?;
        pos += 8;
        let anns = read_annotation_set(buf, ann_off, opts)?;
        field_annotations.push((field_idx, anns));
    }

    let mut method_annotations =
        Vec::with_capacity(methods_size.min(buf.len().saturating_sub(pos) / 8));
    for _ in 0..methods_size {
        let method_idx = MethodIdx(read_u32(buf, pos)?);
        let ann_off = read_u32(buf, pos + 4)?;
        pos += 8;
        let anns = read_annotation_set(buf, ann_off, opts)?;
        method_annotations.push((method_idx, anns));
    }

    let mut parameter_annotations =
        Vec::with_capacity(params_size.min(buf.len().saturating_sub(pos) / 8));
    for _ in 0..params_size {
        let method_idx = MethodIdx(read_u32(buf, pos)?);
        let ann_off = read_u32(buf, pos + 4)?;
        pos += 8;
        let param_anns = read_annotation_set_ref_list(buf, ann_off, opts)?;
        parameter_annotations.push((method_idx, param_anns));
    }

    let class_annotations = if class_annotations_off != 0 {
        read_annotation_set(buf, class_annotations_off, opts)?
    } else {
        Vec::new()
    };

    Ok(AnnotationsDirectory {
        class: class_annotations,
        fields: field_annotations,
        methods: method_annotations,
        parameters: parameter_annotations,
    })
}

pub fn read_annotation_set(
    buf: &[u8],
    off: u32,
    opts: ParseOptions,
) -> Result<Vec<AnnotationItem>> {
    let base = off as usize;
    let size = read_u32(buf, base)? as usize;
    let mut items = Vec::with_capacity(size.min(buf.len().saturating_sub(base + 4) / 4));
    for i in 0..size {
        let item_off = read_u32(buf, base + 4 + i * 4)?;
        items.push(read_annotation_item(buf, item_off, opts)?);
    }
    Ok(items)
}

fn read_annotation_set_ref_list(
    buf: &[u8],
    off: u32,
    opts: ParseOptions,
) -> Result<Vec<Vec<AnnotationItem>>> {
    let base = off as usize;
    let size = read_u32(buf, base)? as usize;
    let mut result = Vec::with_capacity(size.min(buf.len().saturating_sub(base + 4) / 4));
    for i in 0..size {
        let set_off = read_u32(buf, base + 4 + i * 4)?;
        if set_off != 0 {
            result.push(read_annotation_set(buf, set_off, opts)?);
        } else {
            result.push(Vec::new());
        }
    }
    Ok(result)
}

fn read_annotation_item(buf: &[u8], off: u32, opts: ParseOptions) -> Result<AnnotationItem> {
    let pos = off as usize;
    let visibility_byte = read_u8(buf, pos, "annotation item")?;
    let visibility = AnnotationVisibility::from_u8(visibility_byte)
        .ok_or_else(|| invalid_annotation_visibility(visibility_byte))?;

    let (annotation, _size) = read_encoded_annotation_with_opts(buf, pos + 1, opts)?;

    Ok(AnnotationItem {
        visibility,
        type_: annotation.type_,
        elements: annotation
            .elements
            .into_iter()
            .map(|e| AnnotationElement {
                name: e.name,
                value: e.value,
            })
            .collect(),
    })
}
