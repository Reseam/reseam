use super::annotation_reader::read_annotations_directory;
use super::code_reader::read_code_item;
use super::encoded_value_reader::read_encoded_array;
use super::header_reader::u32_at;
use super::id_reader::read_type_list;
use crate::encoding::leb128::read_uleb128;
use crate::error::Result;
use crate::model::access_flags::AccessFlags;
use crate::model::class::{ClassData, ClassDef, EncodedField, EncodedMethod, NO_INDEX};
use crate::model::field::FieldIdx;
use crate::model::method::MethodIdx;
use crate::model::string::StringIdx;
use crate::model::types::TypeIdx;

pub fn read_class_defs(buf: &[u8], off: u32, count: u32) -> Result<Vec<ClassDef>> {
    read_class_defs_impl(buf, off, count, false).map(|(classes, _)| classes)
}

/// Parse class defs with optional lazy mode. Returns (classes, class_data_offsets).
/// When lazy=true, class_data is not parsed; offsets are returned for later resolution.
pub fn read_class_defs_lazy(buf: &[u8], off: u32, count: u32) -> Result<(Vec<ClassDef>, Vec<u32>)> {
    read_class_defs_impl(buf, off, count, true)
}

fn read_class_defs_impl(
    buf: &[u8],
    off: u32,
    count: u32,
    lazy: bool,
) -> Result<(Vec<ClassDef>, Vec<u32>)> {
    let mut classes = Vec::with_capacity(count as usize);
    let mut class_data_offsets = Vec::with_capacity(count as usize);
    let base = off as usize;

    for i in 0..count as usize {
        let entry_off = base + i * 32;
        let class_idx = u32_at(buf, entry_off)?;
        let access_flags_raw = u32_at(buf, entry_off + 4)?;
        let superclass_idx = u32_at(buf, entry_off + 8)?;
        let interfaces_off = u32_at(buf, entry_off + 12)?;
        let source_file_idx = u32_at(buf, entry_off + 16)?;
        let annotations_off = u32_at(buf, entry_off + 20)?;
        let class_data_off = u32_at(buf, entry_off + 24)?;
        let static_values_off = u32_at(buf, entry_off + 28)?;

        let superclass = if superclass_idx == NO_INDEX {
            None
        } else {
            Some(TypeIdx(superclass_idx))
        };

        let interfaces = if interfaces_off != 0 {
            read_type_list(buf, interfaces_off)?
        } else {
            Vec::new()
        };

        let source_file = if source_file_idx == NO_INDEX {
            None
        } else {
            Some(StringIdx(source_file_idx))
        };

        let annotations = if annotations_off != 0 {
            Some(read_annotations_directory(buf, annotations_off)?)
        } else {
            None
        };

        class_data_offsets.push(class_data_off);

        let class_data = if lazy {
            // Defer parsing — will be resolved on demand
            None
        } else if class_data_off != 0 {
            Some(read_class_data(buf, class_data_off)?)
        } else {
            None
        };

        let static_values = if static_values_off != 0 {
            let (values, _) = read_encoded_array(buf, static_values_off as usize)?;
            values
        } else {
            Vec::new()
        };

        classes.push(ClassDef {
            class_type: TypeIdx(class_idx),
            access_flags: AccessFlags::from_bits_retain(access_flags_raw),
            superclass,
            interfaces,
            source_file,
            annotations,
            class_data,
            static_values,
        });
    }

    Ok((classes, class_data_offsets))
}

/// Parse class data at a specific offset. Used by lazy resolution.
pub fn read_class_data_at(buf: &[u8], off: usize) -> Result<ClassData> {
    read_class_data(buf, off as u32)
}

fn read_class_data(buf: &[u8], off: u32) -> Result<ClassData> {
    let mut pos = off as usize;

    let (static_fields_size, n) = read_uleb128(buf, pos)?;
    pos += n;
    let (instance_fields_size, n) = read_uleb128(buf, pos)?;
    pos += n;
    let (direct_methods_size, n) = read_uleb128(buf, pos)?;
    pos += n;
    let (virtual_methods_size, n) = read_uleb128(buf, pos)?;
    pos += n;

    let (static_fields, new_pos) = read_encoded_fields(buf, pos, static_fields_size)?;
    pos = new_pos;
    let (instance_fields, new_pos) = read_encoded_fields(buf, pos, instance_fields_size)?;
    pos = new_pos;
    let (direct_methods, new_pos) = read_encoded_methods(buf, pos, direct_methods_size)?;
    pos = new_pos;
    let (virtual_methods, _) = read_encoded_methods(buf, pos, virtual_methods_size)?;

    Ok(ClassData {
        static_fields,
        instance_fields,
        direct_methods,
        virtual_methods,
    })
}

fn read_encoded_fields(
    buf: &[u8],
    mut pos: usize,
    count: u32,
) -> Result<(Vec<EncodedField>, usize)> {
    let mut fields = Vec::with_capacity(count as usize);
    let mut field_idx: u32 = 0;

    for _ in 0..count {
        let (diff, n) = read_uleb128(buf, pos)?;
        pos += n;
        field_idx = field_idx.wrapping_add(diff);

        let (access, n) = read_uleb128(buf, pos)?;
        pos += n;

        fields.push(EncodedField {
            field: FieldIdx(field_idx),
            access_flags: AccessFlags::from_bits_retain(access),
        });
    }

    Ok((fields, pos))
}

fn read_encoded_methods(
    buf: &[u8],
    mut pos: usize,
    count: u32,
) -> Result<(Vec<EncodedMethod>, usize)> {
    let mut methods = Vec::with_capacity(count as usize);
    let mut method_idx: u32 = 0;

    for _ in 0..count {
        let (diff, n) = read_uleb128(buf, pos)?;
        pos += n;
        method_idx = method_idx.wrapping_add(diff);

        let (access, n) = read_uleb128(buf, pos)?;
        pos += n;

        let (code_off, n) = read_uleb128(buf, pos)?;
        pos += n;

        let code = if code_off != 0 {
            Some(read_code_item(buf, code_off)?)
        } else {
            None
        };

        methods.push(EncodedMethod {
            method: MethodIdx(method_idx),
            access_flags: AccessFlags::from_bits_retain(access),
            code,
        });
    }

    Ok((methods, pos))
}
