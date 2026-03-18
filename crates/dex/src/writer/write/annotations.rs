use super::DexWriter;
use crate::encoding::encoded_value::write_encoded_value;
use crate::encoding::leb128::write_uleb128;
use std::collections::HashMap;

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
pub(crate) fn write_annotations(
    w: &mut DexWriter,
    dex: &crate::model::dex_file::DexFile,
) -> ClassAnnotations {
    let mut annotation_item_cache: HashMap<Vec<u8>, usize> = HashMap::new();
    let mut annotation_items: Vec<Vec<u8>> = Vec::new();
    let mut annotation_set_cache: HashMap<Vec<usize>, usize> = HashMap::new();
    let mut annotation_sets: Vec<Vec<usize>> = Vec::new();
    let mut annotation_ref_lists: Vec<Vec<Option<usize>>> = Vec::new();
    let mut pending_class_ann_datas = Vec::with_capacity(dex.classes.len());

    for class in &dex.classes {
        if let Some(ref ann_dir) = class.annotations {
            let mut cad = PendingClassAnnData {
                class_ann_set: None,
                field_ann: Vec::new(),
                method_ann: Vec::new(),
                param_ann: Vec::new(),
            };

            if !ann_dir.class_annotations.is_empty() {
                cad.class_ann_set = Some(intern_annotation_set(
                    &ann_dir.class_annotations,
                    &mut annotation_item_cache,
                    &mut annotation_items,
                    &mut annotation_set_cache,
                    &mut annotation_sets,
                ));
            }

            for (field_idx, anns) in &ann_dir.field_annotations {
                let set_idx = intern_annotation_set(
                    anns,
                    &mut annotation_item_cache,
                    &mut annotation_items,
                    &mut annotation_set_cache,
                    &mut annotation_sets,
                );
                cad.field_ann.push((field_idx.0, set_idx));
            }

            for (method_idx, anns) in &ann_dir.method_annotations {
                let set_idx = intern_annotation_set(
                    anns,
                    &mut annotation_item_cache,
                    &mut annotation_items,
                    &mut annotation_set_cache,
                    &mut annotation_sets,
                );
                cad.method_ann.push((method_idx.0, set_idx));
            }

            for (method_idx, param_anns) in &ann_dir.parameter_annotations {
                let mut set_idxs = Vec::new();
                for anns in param_anns {
                    if anns.is_empty() {
                        set_idxs.push(None);
                    } else {
                        let set_idx = intern_annotation_set(
                            anns,
                            &mut annotation_item_cache,
                            &mut annotation_items,
                            &mut annotation_set_cache,
                            &mut annotation_sets,
                        );
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

    let ann_items_start = w.pos();
    let mut annotation_item_offsets = Vec::with_capacity(annotation_items.len());
    for item in &annotation_items {
        let off = w.pos();
        w.buf.extend_from_slice(item);
        annotation_item_offsets.push(off);
    }
    if !annotation_items.is_empty() {
        w.map_entries.push(crate::model::map::MapItem {
            type_code: crate::model::map::TYPE_ANNOTATION_ITEM,
            size: annotation_items.len() as u32,
            offset: ann_items_start,
        });
    }

    let mut annotation_set_offsets = Vec::with_capacity(annotation_sets.len());
    if !annotation_sets.is_empty() {
        w.align(4);
        let annotation_set_first_off = w.pos();
        for set in &annotation_sets {
            let set_off = w.pos();
            w.write_u32(set.len() as u32);
            for &item_idx in set {
                w.write_u32(annotation_item_offsets[item_idx]);
            }
            annotation_set_offsets.push(set_off);
        }
        w.map_entries.push(crate::model::map::MapItem {
            type_code: crate::model::map::TYPE_ANNOTATION_SET_ITEM,
            size: annotation_sets.len() as u32,
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
        w.map_entries.push(crate::model::map::MapItem {
            type_code: crate::model::map::TYPE_ANNOTATION_SET_REF_LIST,
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
        w.map_entries.push(crate::model::map::MapItem {
            type_code: crate::model::map::TYPE_ANNOTATIONS_DIRECTORY_ITEM,
            size: annotation_dir_count,
            offset: annotation_dir_first_off,
        });
    } else {
        class_ann_datas.resize_with(pending_class_ann_datas.len(), || None);
    }

    class_ann_datas
}

/// Serializes a single annotation item to its on-disk form.
fn serialize_annotation_item(item: &crate::model::annotation::AnnotationItem) -> Vec<u8> {
    let mut tmp = Vec::new();
    tmp.push(item.visibility.to_u8());
    write_uleb128(&mut tmp, item.type_.0);
    write_uleb128(&mut tmp, item.elements.len() as u32);
    for elem in &item.elements {
        write_uleb128(&mut tmp, elem.name.0);
        write_encoded_value(&mut tmp, &elem.value);
    }
    tmp
}

/// Interns a single annotation item, returning its index in the annotation-item section.
fn intern_annotation_item(
    item: &crate::model::annotation::AnnotationItem,
    item_cache: &mut HashMap<Vec<u8>, usize>,
    items: &mut Vec<Vec<u8>>,
) -> usize {
    let serialized = serialize_annotation_item(item);
    if let Some(&idx) = item_cache.get(&serialized) {
        return idx;
    }

    let idx = items.len();
    item_cache.insert(serialized.clone(), idx);
    items.push(serialized);
    idx
}

/// Interns one annotation set after materializing its member items.
fn intern_annotation_set(
    items: &[crate::model::annotation::AnnotationItem],
    item_cache: &mut HashMap<Vec<u8>, usize>,
    annotation_items: &mut Vec<Vec<u8>>,
    set_cache: &mut HashMap<Vec<usize>, usize>,
    annotation_sets: &mut Vec<Vec<usize>>,
) -> usize {
    let mut item_indices = Vec::with_capacity(items.len());
    for item in items {
        let idx = intern_annotation_item(item, item_cache, annotation_items);
        item_indices.push(idx);
    }
    if let Some(&set_idx) = set_cache.get(&item_indices) {
        return set_idx;
    }

    let set_idx = annotation_sets.len();
    set_cache.insert(item_indices.clone(), set_idx);
    annotation_sets.push(item_indices);
    set_idx
}

#[cfg(test)]
mod tests {
    use super::write_annotations;
    use crate::model::access_flags::AccessFlags;
    use crate::model::annotation::{
        AnnotationElement, AnnotationItem, AnnotationVisibility, AnnotationsDirectory,
    };
    use crate::model::class::ClassDef;
    use crate::model::encoded_value::EncodedValue;
    use crate::model::header::{DexHeader, DexVersion};
    use crate::model::map::{
        TYPE_ANNOTATIONS_DIRECTORY_ITEM, TYPE_ANNOTATION_ITEM, TYPE_ANNOTATION_SET_ITEM,
    };
    use crate::model::string::{DexString, StringIdx};
    use crate::model::types::TypeIdx;
    use crate::writer::write::DexWriter;

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
    fn annotation_sections_are_written_contiguously() {
        let mut dex = crate::model::dex_file::DexFile::new(empty_header());
        dex.strings = vec![
            DexString::new("LA;".to_owned()),
            DexString::new("LB;".to_owned()),
            DexString::new("Ljava/lang/Object;".to_owned()),
            DexString::new("LAnnOne;".to_owned()),
            DexString::new("LAnnTwo;".to_owned()),
            DexString::new("value".to_owned()),
        ];
        dex.types = vec![
            StringIdx(0),
            StringIdx(1),
            StringIdx(2),
            StringIdx(3),
            StringIdx(4),
        ];

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

        dex.classes = vec![
            ClassDef {
                class_type: TypeIdx(0),
                access_flags: AccessFlags::PUBLIC,
                superclass: Some(TypeIdx(2)),
                interfaces: Vec::new(),
                source_file: None,
                annotations: Some(AnnotationsDirectory {
                    class_annotations: vec![class_one_annotation],
                    field_annotations: Vec::new(),
                    method_annotations: Vec::new(),
                    parameter_annotations: Vec::new(),
                }),
                class_data: None,
                static_values: Vec::new(),
            },
            ClassDef {
                class_type: TypeIdx(1),
                access_flags: AccessFlags::PUBLIC,
                superclass: Some(TypeIdx(2)),
                interfaces: Vec::new(),
                source_file: None,
                annotations: Some(AnnotationsDirectory {
                    class_annotations: vec![class_two_annotation],
                    field_annotations: Vec::new(),
                    method_annotations: Vec::new(),
                    parameter_annotations: Vec::new(),
                }),
                class_data: None,
                static_values: Vec::new(),
            },
        ];

        let mut writer = DexWriter::new();
        let class_ann_datas = write_annotations(&mut writer, &dex);

        let ann_item_map = writer
            .map_entries
            .iter()
            .find(|entry| entry.type_code == TYPE_ANNOTATION_ITEM)
            .expect("annotation item map entry");
        let ann_set_map = writer
            .map_entries
            .iter()
            .find(|entry| entry.type_code == TYPE_ANNOTATION_SET_ITEM)
            .expect("annotation set map entry");
        let ann_dir_map = writer
            .map_entries
            .iter()
            .find(|entry| entry.type_code == TYPE_ANNOTATIONS_DIRECTORY_ITEM)
            .expect("annotation directory map entry");

        let mut item_offsets = Vec::new();
        let mut set_offsets = Vec::new();
        let mut dir_offsets = Vec::new();

        for class_ann in class_ann_datas {
            let (dir_off, cad) = class_ann.expect("annotated class");
            dir_offsets.push(dir_off);
            set_offsets.push(cad.class_ann_set_off);

            let set_base = cad.class_ann_set_off as usize;
            let set_size =
                u32::from_le_bytes(writer.buf[set_base..set_base + 4].try_into().unwrap()) as usize;
            for i in 0..set_size {
                let off = set_base + 4 + i * 4;
                item_offsets.push(u32::from_le_bytes(
                    writer.buf[off..off + 4].try_into().unwrap(),
                ));
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
    }
}
