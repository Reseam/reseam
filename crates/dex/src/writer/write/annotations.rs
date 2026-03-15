use super::DexWriter;
use crate::encoding::encoded_value::write_encoded_value;
use crate::encoding::leb128::write_uleb128;

/// Serialized annotation-directory fragments needed for class-def backpatching.
pub(crate) struct ClassAnnData {
    pub(crate) class_ann_set_off: u32,
    pub(crate) field_ann: Vec<(u32, u32)>,
    pub(crate) method_ann: Vec<(u32, u32)>,
    pub(crate) param_ann: Vec<(u32, u32)>,
}

pub(crate) type ClassAnnotations = Vec<Option<(u32, ClassAnnData)>>;

/// Writes annotation items, sets, ref-lists, and directories.
pub(crate) fn write_annotations(
    w: &mut DexWriter,
    dex: &crate::model::dex_file::DexFile,
) -> ClassAnnotations {
    let ann_items_start = w.pos();
    let mut total_ann_items = 0u32;

    let mut class_ann_datas = Vec::new();

    let mut annotation_set_count = 0u32;
    let mut annotation_set_first_off = 0u32;
    let mut annotation_set_ref_count = 0u32;
    let mut annotation_set_ref_first_off = 0u32;
    let mut annotation_dir_count = 0u32;
    let mut annotation_dir_first_off = 0u32;

    for class in &dex.classes {
        if let Some(ref ann_dir) = class.annotations {
            let mut cad = ClassAnnData {
                class_ann_set_off: 0,
                field_ann: Vec::new(),
                method_ann: Vec::new(),
                param_ann: Vec::new(),
            };

            let mut track_set = |off: u32| {
                if annotation_set_count == 0 {
                    annotation_set_first_off = off;
                }
                annotation_set_count += 1;
            };

            if !ann_dir.class_annotations.is_empty() {
                let set_off =
                    write_annotation_set(w, &ann_dir.class_annotations, &mut total_ann_items);
                cad.class_ann_set_off = set_off;
                track_set(set_off);
            }

            for (field_idx, anns) in &ann_dir.field_annotations {
                let set_off = write_annotation_set(w, anns, &mut total_ann_items);
                cad.field_ann.push((field_idx.0, set_off));
                track_set(set_off);
            }

            for (method_idx, anns) in &ann_dir.method_annotations {
                let set_off = write_annotation_set(w, anns, &mut total_ann_items);
                cad.method_ann.push((method_idx.0, set_off));
                track_set(set_off);
            }

            for (method_idx, param_anns) in &ann_dir.parameter_annotations {
                let mut set_offs = Vec::new();
                for anns in param_anns {
                    if anns.is_empty() {
                        set_offs.push(0u32);
                    } else {
                        let set_off = write_annotation_set(w, anns, &mut total_ann_items);
                        set_offs.push(set_off);
                        track_set(set_off);
                    }
                }
                w.align(4);
                let ref_list_off = w.pos();
                if annotation_set_ref_count == 0 {
                    annotation_set_ref_first_off = ref_list_off;
                }
                annotation_set_ref_count += 1;
                w.write_u32(param_anns.len() as u32);
                for so in &set_offs {
                    w.write_u32(*so);
                }
                cad.param_ann.push((method_idx.0, ref_list_off));
            }

            w.align(4);
            let dir_off = w.pos();
            if annotation_dir_count == 0 {
                annotation_dir_first_off = dir_off;
            }
            w.write_u32(cad.class_ann_set_off);
            w.write_u32(cad.field_ann.len() as u32);
            w.write_u32(cad.method_ann.len() as u32);
            w.write_u32(cad.param_ann.len() as u32);
            for (fi, so) in &cad.field_ann {
                w.write_u32(*fi);
                w.write_u32(*so);
            }
            for (mi, so) in &cad.method_ann {
                w.write_u32(*mi);
                w.write_u32(*so);
            }
            for (mi, so) in &cad.param_ann {
                w.write_u32(*mi);
                w.write_u32(*so);
            }
            annotation_dir_count += 1;
            class_ann_datas.push(Some((dir_off, cad)));
        } else {
            class_ann_datas.push(None);
        }
    }

    if total_ann_items > 0 {
        w.map_entries.push(crate::model::map::MapItem {
            type_code: crate::model::map::TYPE_ANNOTATION_ITEM,
            size: total_ann_items,
            offset: ann_items_start,
        });
    }
    if annotation_set_count > 0 {
        w.map_entries.push(crate::model::map::MapItem {
            type_code: crate::model::map::TYPE_ANNOTATION_SET_ITEM,
            size: annotation_set_count,
            offset: annotation_set_first_off,
        });
    }
    if annotation_set_ref_count > 0 {
        w.map_entries.push(crate::model::map::MapItem {
            type_code: crate::model::map::TYPE_ANNOTATION_SET_REF_LIST,
            size: annotation_set_ref_count,
            offset: annotation_set_ref_first_off,
        });
    }
    if annotation_dir_count > 0 {
        w.map_entries.push(crate::model::map::MapItem {
            type_code: crate::model::map::TYPE_ANNOTATIONS_DIRECTORY_ITEM,
            size: annotation_dir_count,
            offset: annotation_dir_first_off,
        });
    }

    class_ann_datas
}

/// Serializes a single annotation item with byte-level deduplication.
fn write_annotation_item(
    w: &mut DexWriter,
    item: &crate::model::annotation::AnnotationItem,
    count: &mut u32,
) -> u32 {
    let mut tmp = Vec::new();
    tmp.push(item.visibility.to_u8());
    write_uleb128(&mut tmp, item.type_.0);
    write_uleb128(&mut tmp, item.elements.len() as u32);
    for elem in &item.elements {
        write_uleb128(&mut tmp, elem.name.0);
        write_encoded_value(&mut tmp, &elem.value);
    }

    if let Some(&cached_off) = w.annotation_item_cache.get(&tmp) {
        return cached_off;
    }

    let off = w.pos();
    w.buf.extend_from_slice(&tmp);
    w.annotation_item_cache.insert(tmp, off);
    *count += 1;
    off
}

/// Serializes one annotation set after materializing its member items.
fn write_annotation_set(
    w: &mut DexWriter,
    items: &[crate::model::annotation::AnnotationItem],
    count: &mut u32,
) -> u32 {
    let mut item_offsets = Vec::with_capacity(items.len());
    for item in items {
        let off = write_annotation_item(w, item, count);
        item_offsets.push(off);
    }
    if let Some(&cached_off) = w.annotation_set_cache.get(&item_offsets) {
        return cached_off;
    }
    w.align(4);
    let set_off = w.pos();
    w.write_u32(items.len() as u32);
    for off in &item_offsets {
        w.write_u32(*off);
    }
    w.annotation_set_cache.insert(item_offsets, set_off);
    set_off
}
