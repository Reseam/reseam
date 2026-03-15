use super::class_reader::read_class_defs;
use super::encoded_value_reader::read_encoded_array;
use super::header_reader::{read_header, u16_at, u32_at};
use super::id_reader::*;
use crate::encoding::leb128::read_uleb128;
use crate::error::{catch_parser_panic, DexError, Result};
use crate::model::call_site::CallSiteItem;
use crate::model::dex_file::DexFile;
use crate::model::encoded_value::EncodedValue;
use crate::model::field::FieldIdx;
use crate::model::header::ParseOptions;
use crate::model::hidden_api::{ClassHiddenApiFlags, HiddenApiData, HiddenApiFlag};
use crate::model::method::MethodIdx;
use crate::model::method_handle::{MethodHandle, MethodHandleMember, MethodHandleType};
use std::sync::Arc;

pub fn parse(buf: &[u8], opts: ParseOptions) -> Result<DexFile> {
    catch_parser_panic("parsing DEX data", || parse_impl(buf, opts))
}

fn parse_impl(buf: &[u8], opts: ParseOptions) -> Result<DexFile> {
    let header = read_header(buf, &opts)?;
    let lazy = opts.lazy;
    let mut dex = DexFile::new(header.clone());

    // Retain the raw buffer for lazy resolution and caller access.
    dex.raw = Some(Arc::from(buf));

    // String IDs -> String Data
    if header.string_ids_size > 0 {
        let string_offsets = read_string_ids(buf, header.string_ids_off, header.string_ids_size)?;
        for &off in &string_offsets {
            let s = read_string_data(buf, off)?;
            dex.strings.push(s);
        }
    }

    // Type IDs
    if header.type_ids_size > 0 {
        dex.types = read_type_ids(buf, header.type_ids_off, header.type_ids_size)?;
    }

    // Proto IDs
    if header.proto_ids_size > 0 {
        dex.prototypes = read_proto_ids(buf, header.proto_ids_off, header.proto_ids_size)?;
    }

    // Field IDs
    if header.field_ids_size > 0 {
        dex.fields = read_field_ids(buf, header.field_ids_off, header.field_ids_size)?;
    }

    // Method IDs
    if header.method_ids_size > 0 {
        dex.methods = read_method_ids(buf, header.method_ids_off, header.method_ids_size)?;
    }

    // Class Defs
    if header.class_defs_size > 0 {
        if lazy {
            let (classes, offsets) = super::class_reader::read_class_defs_lazy(
                buf,
                header.class_defs_off,
                header.class_defs_size,
            )?;
            dex.classes = classes;
            dex.lazy_class_data_offsets = Some(offsets);
        } else {
            dex.classes = read_class_defs(buf, header.class_defs_off, header.class_defs_size)?;
        }
    }

    // Parse map list to find optional sections
    let map_off = header.map_off as usize;
    let map_size = u32_at(buf, map_off) as usize;

    let mut call_site_off: Option<(u32, u32)> = None;
    let mut method_handle_off: Option<(u32, u32)> = None;
    let mut hidden_api_off: Option<(u32, u32)> = None;

    for i in 0..map_size {
        let entry = map_off + 4 + i * 12;
        let type_code = u16_at(buf, entry);
        let size = u32_at(buf, entry + 4);
        let offset = u32_at(buf, entry + 8);

        match type_code {
            0x0007 => call_site_off = Some((offset, size)),
            0x0008 => method_handle_off = Some((offset, size)),
            0xF000 => hidden_api_off = Some((offset, size)),
            _ => {}
        }
    }

    // DEX 038+: method handles and call sites
    if header.version.supports_call_sites() {
        read_method_handles(buf, method_handle_off, &mut dex)?;
        read_call_sites(buf, call_site_off, &mut dex)?;
    }

    // DEX 039+: hidden API data
    if header.version.supports_hidden_api() {
        if let Some((off, _)) = hidden_api_off {
            dex.hidden_api = Some(read_hidden_api(buf, off as usize, &dex)?);
        }
    }

    dex.build_lookups();
    Ok(dex)
}

fn read_method_handles(
    buf: &[u8],
    method_handle_off: Option<(u32, u32)>,
    dex: &mut DexFile,
) -> Result<()> {
    if let Some((off, count)) = method_handle_off {
        for i in 0..count as usize {
            let base = off as usize + i * 8;
            let handle_type_raw = u16_at(buf, base);
            let member_id = u16_at(buf, base + 4);

            let handle_type = MethodHandleType::from_u16(handle_type_raw).ok_or(
                DexError::InvalidMethodHandleType {
                    value: handle_type_raw,
                },
            )?;

            let member = if handle_type.is_field() {
                MethodHandleMember::Field(FieldIdx(member_id as u32))
            } else {
                MethodHandleMember::Method(MethodIdx(member_id as u32))
            };

            dex.method_handles.push(MethodHandle {
                handle_type,
                member,
            });
        }
    }
    Ok(())
}

fn read_call_sites(buf: &[u8], call_site_off: Option<(u32, u32)>, dex: &mut DexFile) -> Result<()> {
    if let Some((off, count)) = call_site_off {
        for i in 0..count as usize {
            let cs_off = u32_at(buf, off as usize + i * 4);
            let (values, _) = read_encoded_array(buf, cs_off as usize)?;

            if values.len() < 3 {
                return Err(DexError::InvalidCallSite {
                    index: i as u32,
                    detail: format!("expected at least 3 elements, got {}", values.len()),
                });
            }

            let bootstrap_method = match &values[0] {
                EncodedValue::MethodHandle(idx) => *idx,
                _ => {
                    return Err(DexError::InvalidCallSite {
                        index: i as u32,
                        detail: "bootstrap method is not a method handle".to_owned(),
                    });
                }
            };
            let method_name = match &values[1] {
                EncodedValue::String(idx) => *idx,
                _ => {
                    return Err(DexError::InvalidCallSite {
                        index: i as u32,
                        detail: "method name is not a string".to_owned(),
                    });
                }
            };
            let method_type = match &values[2] {
                EncodedValue::MethodType(idx) => *idx,
                _ => {
                    return Err(DexError::InvalidCallSite {
                        index: i as u32,
                        detail: "method type is not a method type".to_owned(),
                    });
                }
            };

            let extra_arguments = values[3..].to_vec();

            dex.call_sites.push(CallSiteItem {
                bootstrap_method,
                method_name,
                method_type,
                extra_arguments,
            });
        }
    }
    Ok(())
}

fn read_hidden_api(buf: &[u8], off: usize, dex: &DexFile) -> Result<HiddenApiData> {
    let class_count = dex.classes.len();
    let mut class_flags = Vec::with_capacity(class_count);

    // Read offset table: one u32 per class_def
    let mut data_offsets = Vec::with_capacity(class_count);
    for i in 0..class_count {
        data_offsets.push(u32_at(buf, off + i * 4));
    }

    // For each class, read the flag sequences
    for (i, &data_off) in data_offsets.iter().enumerate() {
        if data_off == 0 {
            class_flags.push(None);
            continue;
        }

        let class = &dex.classes[i];
        let data = match class.class_data.as_ref() {
            Some(d) => d,
            None => {
                class_flags.push(None);
                continue;
            }
        };

        let abs_off = off + data_off as usize;
        let mut pos = abs_off;

        let mut read_flags = |count: usize| -> Result<Vec<HiddenApiFlag>> {
            let mut flags = Vec::with_capacity(count);
            for _ in 0..count {
                let (v, consumed) = read_uleb128(buf, pos)?;
                pos += consumed;
                flags.push(
                    HiddenApiFlag::from_u32(v)
                        .ok_or(DexError::InvalidHiddenApiFlag { value: v })?,
                );
            }
            Ok(flags)
        };

        let static_field_flags = read_flags(data.static_fields.len())?;
        let instance_field_flags = read_flags(data.instance_fields.len())?;
        let direct_method_flags = read_flags(data.direct_methods.len())?;
        let virtual_method_flags = read_flags(data.virtual_methods.len())?;

        class_flags.push(Some(ClassHiddenApiFlags {
            static_field_flags,
            instance_field_flags,
            direct_method_flags,
            virtual_method_flags,
        }));
    }

    Ok(HiddenApiData { class_flags })
}

#[cfg(test)]
mod tests {
    use crate::encoding::encoded_value::write_encoded_array;
    use crate::model::access_flags::AccessFlags;
    use crate::model::class::{ClassData, ClassDef, EncodedField};
    use crate::model::field::FieldIdx;
    use crate::model::header::{DexHeader, DexVersion};
    use crate::model::types::TypeIdx;

    use super::*;

    fn empty_header() -> DexHeader {
        DexHeader {
            version: DexVersion::V039,
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
        }
    }

    #[test]
    fn read_call_sites_rejects_short_entries() {
        let mut buf = vec![0; 4];
        buf[..4].copy_from_slice(&4u32.to_le_bytes());
        write_encoded_array(
            &mut buf,
            &[
                EncodedValue::MethodHandle(crate::model::method_handle::MethodHandleIdx(0)),
                EncodedValue::String(crate::model::string::StringIdx(0)),
            ],
        );

        let mut dex = DexFile::new(empty_header());
        let error = read_call_sites(&buf, Some((0, 1)), &mut dex).unwrap_err();

        assert!(matches!(error, DexError::InvalidCallSite { index: 0, .. }));
    }

    #[test]
    fn read_hidden_api_rejects_unknown_flags() {
        let mut dex = DexFile::new(empty_header());
        dex.classes.push(ClassDef {
            class_type: TypeIdx(0),
            access_flags: AccessFlags::empty(),
            superclass: None,
            interfaces: Vec::new(),
            source_file: None,
            annotations: None,
            class_data: Some(ClassData {
                static_fields: vec![EncodedField {
                    field: FieldIdx(0),
                    access_flags: AccessFlags::empty(),
                }],
                instance_fields: Vec::new(),
                direct_methods: Vec::new(),
                virtual_methods: Vec::new(),
            }),
            static_values: Vec::new(),
        });

        let buf = [4, 0, 0, 0, 99];
        let error = read_hidden_api(&buf, 0, &dex).unwrap_err();

        assert_eq!(
            error.to_string(),
            DexError::InvalidHiddenApiFlag { value: 99 }.to_string()
        );
    }
}
