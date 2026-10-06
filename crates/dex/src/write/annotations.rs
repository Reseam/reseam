// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::encoded_value::write_encoded_value;
use super::intern::{ByteInterner, StreamInterner};
use super::plan::WritePlan;
use super::sink::DexSink;
use crate::encoding::leb128::write_uleb128;
use crate::error::Result;

struct PendingClassAnnData {
    class_ann_set: Option<usize>,
    field_ann: Vec<(u32, usize)>,
    method_ann: Vec<(u32, usize)>,
    param_ann: Vec<(u32, usize)>,
}

pub(crate) fn write_annotations<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
) -> Result<Vec<u32>> {
    let ann_items_start = w.pos();
    let mut items = StreamInterner::default();
    let mut sets = ByteInterner::new()?;
    let mut annotation_ref_lists: Vec<Vec<Option<usize>>> = Vec::new();
    let mut encoded = Vec::new();
    let mut pending_class_ann_datas = Vec::with_capacity(plan.classes.len());

    for k in 0..plan.classes.len() {
        if let Some(ann_dir) = plan.class_annotations(k)? {
            let mut cad = PendingClassAnnData {
                class_ann_set: None,
                field_ann: Vec::new(),
                method_ann: Vec::new(),
                param_ann: Vec::new(),
            };

            if !ann_dir.class.is_empty() {
                cad.class_ann_set = Some(intern_annotation_set(
                    w,
                    &ann_dir.class,
                    &mut items,
                    &mut sets,
                    &mut encoded,
                )?);
            }

            for (field_idx, anns) in &ann_dir.fields {
                let set_idx = intern_annotation_set(w, anns, &mut items, &mut sets, &mut encoded)?;
                cad.field_ann.push((field_idx.0, set_idx));
            }

            for (method_idx, anns) in &ann_dir.methods {
                let set_idx = intern_annotation_set(w, anns, &mut items, &mut sets, &mut encoded)?;
                cad.method_ann.push((method_idx.0, set_idx));
            }

            for (method_idx, param_anns) in &ann_dir.parameters {
                let mut set_idxs = Vec::new();
                for anns in param_anns {
                    if anns.is_empty() {
                        set_idxs.push(None);
                    } else {
                        let set_idx =
                            intern_annotation_set(w, anns, &mut items, &mut sets, &mut encoded)?;
                        set_idxs.push(Some(set_idx));
                    }
                }
                let ref_list_idx = annotation_ref_lists.len();
                annotation_ref_lists.push(set_idxs);
                cad.param_ann.push((method_idx.0, ref_list_idx));
            }

            pending_class_ann_datas.push(Some(cad));
        } else {
            pending_class_ann_datas.push(None);
        }
    }

    if items.len() > 0 {
        w.map_entries.push(crate::types::map::MapItem {
            type_code: crate::types::map::TYPE_ANNOTATION_ITEM,
            size: items.len() as u32,
            offset: ann_items_start,
        });
    }

    let annotation_set_offsets = write_sets(w, &sets)?;

    let mut annotation_ref_list_offsets = Vec::with_capacity(annotation_ref_lists.len());
    if !annotation_ref_lists.is_empty() {
        w.align(4);
        let annotation_set_ref_first_off = w.pos();
        for ref_list in &annotation_ref_lists {
            let ref_list_off = w.pos();
            w.write_u32(ref_list.len() as u32);
            for set_idx in ref_list {
                let set_off = set_idx.map_or(0, |idx| annotation_set_offsets[idx]);
                w.write_u32(set_off);
            }
            annotation_ref_list_offsets.push(ref_list_off);
        }
        w.map_entries.push(crate::types::map::MapItem {
            type_code: crate::types::map::TYPE_ANNOTATION_SET_REF_LIST,
            size: annotation_ref_lists.len() as u32,
            offset: annotation_set_ref_first_off,
        });
    }

    Ok(write_directories(
        w,
        pending_class_ann_datas,
        &annotation_set_offsets,
        &annotation_ref_list_offsets,
    ))
}

fn write_sets<S: DexSink>(w: &mut DexWriter<S>, sets: &ByteInterner) -> Result<Vec<u32>> {
    let mut annotation_set_offsets = Vec::with_capacity(sets.len());
    if !sets.is_empty() {
        w.align(4);
        let annotation_set_first_off = w.pos();
        for set_idx in 0..sets.len() {
            let mut set = Vec::new();
            sets.get(set_idx, &mut set)?;
            let set_off = w.pos();
            w.write_u32((set.len() / 4) as u32);
            for item_off in set.as_chunks::<4>().0 {
                w.write_u32(u32::from_le_bytes(*item_off));
            }
            annotation_set_offsets.push(set_off);
        }
        w.map_entries.push(crate::types::map::MapItem {
            type_code: crate::types::map::TYPE_ANNOTATION_SET_ITEM,
            size: sets.len() as u32,
            offset: annotation_set_first_off,
        });
    }

    Ok(annotation_set_offsets)
}

fn write_directories<S: DexSink>(
    w: &mut DexWriter<S>,
    pending_class_ann_datas: Vec<Option<PendingClassAnnData>>,
    annotation_set_offsets: &[u32],
    annotation_ref_list_offsets: &[u32],
) -> Vec<u32> {
    let mut class_ann_datas = Vec::with_capacity(pending_class_ann_datas.len());
    let annotation_dir_count = pending_class_ann_datas
        .iter()
        .filter(|cad| cad.is_some())
        .count() as u32;
    if annotation_dir_count > 0 {
        w.align(4);
        let annotation_dir_first_off = w.pos();
        for cad in pending_class_ann_datas {
            if let Some(cad) = cad {
                let dir_off = w.pos();
                w.write_u32(
                    cad.class_ann_set
                        .map_or(0, |idx| annotation_set_offsets[idx]),
                );
                w.write_u32(cad.field_ann.len() as u32);
                w.write_u32(cad.method_ann.len() as u32);
                w.write_u32(cad.param_ann.len() as u32);
                for (member, set) in cad.field_ann.into_iter().chain(cad.method_ann) {
                    w.write_u32(member);
                    w.write_u32(annotation_set_offsets[set]);
                }
                for (member, list) in cad.param_ann {
                    w.write_u32(member);
                    w.write_u32(annotation_ref_list_offsets[list]);
                }
                class_ann_datas.push(dir_off);
            } else {
                class_ann_datas.push(0);
            }
        }
        w.map_entries.push(crate::types::map::MapItem {
            type_code: crate::types::map::TYPE_ANNOTATIONS_DIRECTORY_ITEM,
            size: annotation_dir_count,
            offset: annotation_dir_first_off,
        });
    } else {
        class_ann_datas.resize(pending_class_ann_datas.len(), 0);
    }

    class_ann_datas
}

fn serialize_annotation_item(out: &mut Vec<u8>, item: &crate::types::annotation::AnnotationItem) {
    out.clear();
    out.push(item.visibility.to_u8());
    write_uleb128(out, item.type_.0);
    write_uleb128(out, item.elements.len() as u32);
    for elem in &item.elements {
        write_uleb128(out, elem.name.0);
        write_encoded_value(out, &elem.value);
    }
}

fn intern_annotation_set<S: DexSink>(
    w: &mut DexWriter<S>,
    annotations: &[crate::types::annotation::AnnotationItem],
    items: &mut StreamInterner,
    sets: &mut ByteInterner,
    encoded: &mut Vec<u8>,
) -> Result<usize> {
    let mut key = Vec::with_capacity(annotations.len() * 4);
    for item in annotations {
        serialize_annotation_item(encoded, item);
        let offset = items.intern(&mut w.sink, encoded)?;
        key.extend_from_slice(&offset.to_le_bytes());
    }
    sets.intern(&key)
}
