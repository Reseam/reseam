use super::encoded_value::read_encoded_annotation;
use super::header::u32_at;
use crate::error::{invalid_annotation_visibility, read_u8, Result};
use crate::types::annotation::{
    AnnotationElement, AnnotationItem, AnnotationVisibility, AnnotationsDirectory,
};
use crate::types::{FieldIdx, MethodIdx};

pub fn read_annotations_directory(buf: &[u8], off: u32) -> Result<AnnotationsDirectory> {
    let base = off as usize;
    let class_annotations_off = u32_at(buf, base)?;
    let fields_size = u32_at(buf, base + 4)? as usize;
    let methods_size = u32_at(buf, base + 8)? as usize;
    let params_size = u32_at(buf, base + 12)? as usize;

    let mut pos = base + 16;

    let mut field_annotations = Vec::with_capacity(fields_size);
    for _ in 0..fields_size {
        let field_idx = FieldIdx(u32_at(buf, pos)?);
        let ann_off = u32_at(buf, pos + 4)?;
        pos += 8;
        let anns = read_annotation_set(buf, ann_off)?;
        field_annotations.push((field_idx, anns));
    }

    let mut method_annotations = Vec::with_capacity(methods_size);
    for _ in 0..methods_size {
        let method_idx = MethodIdx(u32_at(buf, pos)?);
        let ann_off = u32_at(buf, pos + 4)?;
        pos += 8;
        let anns = read_annotation_set(buf, ann_off)?;
        method_annotations.push((method_idx, anns));
    }

    let mut parameter_annotations = Vec::with_capacity(params_size);
    for _ in 0..params_size {
        let method_idx = MethodIdx(u32_at(buf, pos)?);
        let ann_off = u32_at(buf, pos + 4)?;
        pos += 8;
        let param_anns = read_annotation_set_ref_list(buf, ann_off)?;
        parameter_annotations.push((method_idx, param_anns));
    }

    let class_annotations = if class_annotations_off != 0 {
        read_annotation_set(buf, class_annotations_off)?
    } else {
        Vec::new()
    };

    Ok(AnnotationsDirectory {
        class_annotations,
        field_annotations,
        method_annotations,
        parameter_annotations,
    })
}

pub fn read_annotation_set(buf: &[u8], off: u32) -> Result<Vec<AnnotationItem>> {
    let base = off as usize;
    let size = u32_at(buf, base)? as usize;
    let mut items = Vec::with_capacity(size);
    for i in 0..size {
        let item_off = u32_at(buf, base + 4 + i * 4)?;
        items.push(read_annotation_item(buf, item_off)?);
    }
    Ok(items)
}

fn read_annotation_set_ref_list(buf: &[u8], off: u32) -> Result<Vec<Vec<AnnotationItem>>> {
    let base = off as usize;
    let size = u32_at(buf, base)? as usize;
    let mut result = Vec::with_capacity(size);
    for i in 0..size {
        let set_off = u32_at(buf, base + 4 + i * 4)?;
        if set_off != 0 {
            result.push(read_annotation_set(buf, set_off)?);
        } else {
            result.push(Vec::new());
        }
    }
    Ok(result)
}

fn read_annotation_item(buf: &[u8], off: u32) -> Result<AnnotationItem> {
    let pos = off as usize;
    let visibility_byte = read_u8(buf, pos, "annotation item")?;
    let visibility = AnnotationVisibility::from_u8(visibility_byte)
        .ok_or_else(|| invalid_annotation_visibility(visibility_byte))?;

    let (annotation, _size) = read_encoded_annotation(buf, pos + 1)?;

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
