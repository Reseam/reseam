// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::encoded_value::write_encoded_value;
use super::intern::{ByteInterner, StreamInterner};
use super::plan::WritePlan;
use super::sink::DexSink;
use super::DexWriter;
use crate::encoding::leb128::write_uleb128;
use crate::error::Result;

/// Serialized annotation-directory fragments needed for class-def backpatching.
pub(crate) struct ClassAnnData {
    pub(crate) class_ann_set_off: u32,
    pub(crate) field_ann: Vec<(u32, u32)>,
    pub(crate) method_ann: Vec<(u32, u32)>,
    pub(crate) param_ann: Vec<(u32, u32)>,
}

pub(crate) type ClassAnnotations = Vec<Option<(u32, ClassAnnData)>>;

struct PendingClassAnnData {
    class_ann_set: Option<usize>,
    field_ann: Vec<(u32, usize)>,
    method_ann: Vec<(u32, usize)>,
    param_ann: Vec<(u32, usize)>,
}

/// Writes annotation items, sets, ref-lists, and directories.
pub(crate) fn write_annotations<S: DexSink>(w: &mut DexWriter<S>, plan: &WritePlan<'_>) -> Result<ClassAnnotations> {
    let ann_items_start = w.pos();
    let mut items = StreamInterner::default();
    let mut sets = ByteInterner::default();
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

            if !ann_dir.class_annotations.is_empty() {
                cad.class_ann_set = Some(intern_annotation_set(w, &ann_dir.class_annotations, &mut items, &mut sets, &mut encoded)?);
            }

            for (field_idx, anns) in &ann_dir.field_annotations {
                let set_idx = intern_annotation_set(w, anns, &mut items, &mut sets, &mut encoded)?;
                cad.field_ann.push((field_idx.0, set_idx));
            }

            for (method_idx, anns) in &ann_dir.method_annotations {
                let set_idx = intern_annotation_set(w, anns, &mut items, &mut sets, &mut encoded)?;
                cad.method_ann.push((method_idx.0, set_idx));
            }

            for (method_idx, param_anns) in &ann_dir.parameter_annotations {
                let mut set_idxs = Vec::new();
                for anns in param_anns {
                    if anns.is_empty() {
                        set_idxs.push(None);
                    } else {
                        let set_idx = intern_annotation_set(w, anns, &mut items, &mut sets, &mut encoded)?;
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

    let mut annotation_set_offsets = Vec::with_capacity(sets.len());
    if !sets.is_empty() {
        w.align(4);
        let annotation_set_first_off = w.pos();
        for set_idx in 0..sets.len() {
            let set = sets.get(set_idx);
            let set_off = w.pos();
            w.write_u32((set.len() / 4) as u32);
            for item_off in set.chunks_exact(4) {
                w.write_u32(u32::from_le_bytes(item_off.try_into().unwrap()));
            }
            annotation_set_offsets.push(set_off);
        }
        w.map_entries.push(crate::types::map::MapItem {
            type_code: crate::types::map::TYPE_ANNOTATION_SET_ITEM,
            size: sets.len() as u32,
            offset: annotation_set_first_off,
        });
    }

    let mut annotation_ref_list_offsets = Vec::with_capacity(annotation_ref_lists.len());
    if !annotation_ref_lists.is_empty() {
        w.align(4);
        let annotation_set_ref_first_off = w.pos();
        for ref_list in &annotation_ref_lists {
            let ref_list_off = w.pos();
            w.write_u32(ref_list.len() as u32);
            for set_idx in ref_list {
                let set_off = set_idx.map(|idx| annotation_set_offsets[idx]).unwrap_or(0);
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
                let realized = ClassAnnData {
                    class_ann_set_off: cad
                        .class_ann_set
                        .map(|idx| annotation_set_offsets[idx])
                        .unwrap_or(0),
                    field_ann: cad
                        .field_ann
                        .into_iter()
                        .map(|(field_idx, set_idx)| (field_idx, annotation_set_offsets[set_idx]))
                        .collect(),
                    method_ann: cad
                        .method_ann
                        .into_iter()
                        .map(|(method_idx, set_idx)| (method_idx, annotation_set_offsets[set_idx]))
                        .collect(),
                    param_ann: cad
                        .param_ann
                        .into_iter()
                        .map(|(method_idx, ref_idx)| {
                            (method_idx, annotation_ref_list_offsets[ref_idx])
                        })
                        .collect(),
                };

                let dir_off = w.pos();
                w.write_u32(realized.class_ann_set_off);
                w.write_u32(realized.field_ann.len() as u32);
                w.write_u32(realized.method_ann.len() as u32);
                w.write_u32(realized.param_ann.len() as u32);
                for (fi, so) in &realized.field_ann {
                    w.write_u32(*fi);
                    w.write_u32(*so);
                }
                for (mi, so) in &realized.method_ann {
                    w.write_u32(*mi);
                    w.write_u32(*so);
                }
                for (mi, so) in &realized.param_ann {
                    w.write_u32(*mi);
                    w.write_u32(*so);
                }
                class_ann_datas.push(Some((dir_off, realized)));
            } else {
                class_ann_datas.push(None);
            }
        }
        w.map_entries.push(crate::types::map::MapItem {
            type_code: crate::types::map::TYPE_ANNOTATIONS_DIRECTORY_ITEM,
            size: annotation_dir_count,
            offset: annotation_dir_first_off,
        });
    } else {
        class_ann_datas.resize_with(pending_class_ann_datas.len(), || None);
    }

    Ok(class_ann_datas)
}

/// Serializes a single annotation item to its on-disk form.
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

/// Writes the set's items (deduplicated) and interns the set as the
/// little-endian item offsets it holds. Returns the set index.
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
    Ok(sets.intern(&key))
}

#[cfg(test)]
mod tests {
    use super::write_annotations;
    use crate::types::access_flags::AccessFlags;
    use crate::types::annotation::{
        AnnotationElement, AnnotationItem, AnnotationVisibility, AnnotationsDirectory,
    };
    use crate::types::class::ClassDef;
    use crate::types::encoded_value::EncodedValue;
    use crate::types::header::{DexHeader, DexVersion};
    use crate::types::map::{
        TYPE_ANNOTATIONS_DIRECTORY_ITEM, TYPE_ANNOTATION_ITEM, TYPE_ANNOTATION_SET_ITEM,
    };
    use crate::types::{StringIdx, TypeIdx};
    use crate::write::DexWriter;

    fn empty_header() -> DexHeader {
        DexHeader {
            version: DexVersion::V035,
            checksum: 0,
            signature: [0; 20],
            file_size: 0,
            link_size: 0,
            link_off: 0,
            map_off: 0,
            string_ids_size: 0,
            string_ids_off: 0,
            type_ids_size: 0,
            type_ids_off: 0,
            proto_ids_size: 0,
            proto_ids_off: 0,
            field_ids_size: 0,
            field_ids_off: 0,
            method_ids_size: 0,
            method_ids_off: 0,
            class_defs_size: 0,
            class_defs_off: 0,
            data_size: 0,
            data_off: 0,
            container_size: 0,
            header_offset: 0,
        }
    }

    #[test]
    fn annotation_sections_are_written_contiguously() -> crate::error::Result<()> {
        let mut dex = crate::file::DexFile::new(empty_header());
        dex.strings = ["LA;", "LB;", "Ljava/lang/Object;", "LAnnOne;", "LAnnTwo;", "value"]
            .into_iter()
            .collect();
        dex.types = crate::file::IdTable::from_vec(vec![
            StringIdx(0),
            StringIdx(1),
            StringIdx(2),
            StringIdx(3),
            StringIdx(4),
        ]);

        let class_one_annotation = AnnotationItem {
            visibility: AnnotationVisibility::Runtime,
            type_: TypeIdx(3),
            elements: vec![AnnotationElement {
                name: StringIdx(5),
                value: EncodedValue::Int(1),
            }],
        };
        let class_two_annotation = AnnotationItem {
            visibility: AnnotationVisibility::Runtime,
            type_: TypeIdx(4),
            elements: vec![AnnotationElement {
                name: StringIdx(5),
                value: EncodedValue::Int(2),
            }],
        };

        dex.classes = crate::file::ClassTable::from_defs(vec![
            ClassDef {
                class_type: TypeIdx(0),
                access_flags: AccessFlags::PUBLIC,
                superclass: Some(TypeIdx(2)),
                interfaces: crate::types::TypeList::new(),
                source_file: None,
                annotations: Some(Box::new(AnnotationsDirectory {
                    class_annotations: vec![class_one_annotation],
                    field_annotations: Vec::new(),
                    method_annotations: Vec::new(),
                    parameter_annotations: Vec::new(),
                })),
                class_data: None,
                static_values: Vec::new(),
            },
            ClassDef {
                class_type: TypeIdx(1),
                access_flags: AccessFlags::PUBLIC,
                superclass: Some(TypeIdx(2)),
                interfaces: crate::types::TypeList::new(),
                source_file: None,
                annotations: Some(Box::new(AnnotationsDirectory {
                    class_annotations: vec![class_two_annotation],
                    field_annotations: Vec::new(),
                    method_annotations: Vec::new(),
                    parameter_annotations: Vec::new(),
                })),
                class_data: None,
                static_values: Vec::new(),
            },
        ]);

        let plan = crate::write::plan::WritePlan::new(&dex)?;
        let mut writer = DexWriter::new(Vec::new());
        let class_ann_datas = write_annotations(&mut writer, &plan)?;

        let ann_item_map = writer
            .map_entries
            .iter()
            .find(|entry| entry.type_code == TYPE_ANNOTATION_ITEM)
            .ok_or_else(|| {
                crate::error::malformed("map", 0, "annotation item map entry missing")
            })?;
        let ann_set_map = writer
            .map_entries
            .iter()
            .find(|entry| entry.type_code == TYPE_ANNOTATION_SET_ITEM)
            .ok_or_else(|| crate::error::malformed("map", 0, "annotation set map entry missing"))?;
        let ann_dir_map = writer
            .map_entries
            .iter()
            .find(|entry| entry.type_code == TYPE_ANNOTATIONS_DIRECTORY_ITEM)
            .ok_or_else(|| {
                crate::error::malformed("map", 0, "annotation directory map entry missing")
            })?;

        let mut item_offsets = Vec::new();
        let mut set_offsets = Vec::new();
        let mut dir_offsets = Vec::new();

        for class_ann in class_ann_datas {
            let (dir_off, cad) = class_ann.ok_or_else(|| {
                crate::error::malformed("annotations", 0, "annotated class missing")
            })?;
            dir_offsets.push(dir_off);
            set_offsets.push(cad.class_ann_set_off);

            let set_base = cad.class_ann_set_off as usize;
            let set_bytes: [u8; 4] = writer
                .sink
                .get(set_base..set_base + 4)
                .and_then(|s| s.try_into().ok())
                .ok_or_else(|| {
                    crate::error::malformed("annotations", set_base, "set size truncated")
                })?;
            let set_size = u32::from_le_bytes(set_bytes) as usize;
            for i in 0..set_size {
                let off = set_base + 4 + i * 4;
                let item_bytes: [u8; 4] = writer
                    .sink
                    .get(off..off + 4)
                    .and_then(|s| s.try_into().ok())
                    .ok_or_else(|| {
                        crate::error::malformed("annotations", off, "item offset truncated")
                    })?;
                item_offsets.push(u32::from_le_bytes(item_bytes));
            }
        }

        assert_eq!(ann_item_map.size, item_offsets.len() as u32);
        assert_eq!(ann_set_map.size, set_offsets.len() as u32);
        assert_eq!(ann_dir_map.size, dir_offsets.len() as u32);
        assert!(item_offsets
            .iter()
            .all(|&off| off >= ann_item_map.offset && off < ann_set_map.offset));
        assert!(set_offsets
            .iter()
            .all(|&off| off >= ann_set_map.offset && off < ann_dir_map.offset));
        assert!(dir_offsets.iter().all(|&off| off >= ann_dir_map.offset));
        Ok(())
    }
}
