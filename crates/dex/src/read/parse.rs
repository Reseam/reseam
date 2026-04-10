use super::class::read_class_defs;
use super::encoded_value::read_encoded_array;
use super::header::{read_header, read_header_at, u16_at, u32_at};
use super::ids::*;
use crate::encoding::leb128::read_uleb128;
use crate::error::{
    invalid_call_site, invalid_hidden_api_flag, invalid_method_handle_type, Result,
};
use crate::file::DexFile;
use crate::types::encoded_value::EncodedValue;
use crate::types::header::ParseOptions;
use crate::types::hidden_api::{ClassHiddenApiFlags, HiddenApiData, HiddenApiFlag};
use crate::types::method_handle::{MethodHandle, MethodHandleMember, MethodHandleType};
use crate::types::{FieldIdx, MethodIdx};
use std::sync::Arc;
use tracing::{debug, instrument};

#[instrument(level = "debug", skip(buf), fields(buffer_len = buf.len(), lazy = opts.lazy))]
pub fn parse(buf: &[u8], opts: ParseOptions) -> Result<DexFile> {
    parse_single(buf, &opts, None)
}

/// Parses a v41 container buffer into its constituent logical DEX files.
///
/// For non-container buffers (v40 and earlier), returns a single-element vec.
#[instrument(level = "debug", skip(buf), fields(buffer_len = buf.len(), lazy = opts.lazy))]
pub fn parse_container(buf: &[u8], opts: ParseOptions) -> Result<Vec<DexFile>> {
    if buf.len() < 8 {
        let dex = parse_single(buf, &opts, None)?;
        return Ok(vec![dex]);
    }

    let mut magic = [0u8; 8];
    magic.copy_from_slice(&buf[..8]);
    let version = crate::types::header::DexVersion::from_magic(&magic);

    let is_container = version.is_some_and(|v| v.is_container_format());
    if !is_container {
        let dex = parse_single(buf, &opts, None)?;
        return Ok(vec![dex]);
    }

    let shared_buf = Arc::from(buf);
    let mut dex_files = Vec::new();
    let mut offset = 0usize;

    while offset < buf.len() {
        if offset + 8 > buf.len() {
            break;
        }
        let mut hdr_magic = [0u8; 8];
        hdr_magic.copy_from_slice(&buf[offset..offset + 8]);
        if crate::types::header::DexVersion::from_magic(&hdr_magic).is_none() {
            break;
        }

        let mut dex = parse_single(buf, &opts, Some(offset))?;
        dex.raw = Some(Arc::clone(&shared_buf));
        dex_files.push(dex);

        offset += dex_files.last().map_or(0, |d| d.header.file_size as usize);
    }

    debug!(dex_count = dex_files.len(), "parsed DEX container");
    Ok(dex_files)
}

fn parse_single(buf: &[u8], opts: &ParseOptions, header_off: Option<usize>) -> Result<DexFile> {
    let header = match header_off {
        Some(off) => read_header_at(buf, off, buf.len() as u32, opts)?,
        None => read_header(buf, opts)?,
    };

    let lazy = opts.lazy;
    let mut dex = DexFile::new(header.clone());
    dex.raw = Some(Arc::from(buf));

    if header.string_ids_size > 0 {
        let string_offsets = read_string_ids(buf, header.string_ids_off, header.string_ids_size)?;
        for &off in &string_offsets {
            let s = read_string_data(buf, off)?;
            dex.strings.push(s);
        }
    }

    if header.type_ids_size > 0 {
        dex.types = read_type_ids(buf, header.type_ids_off, header.type_ids_size)?;
    }

    if header.proto_ids_size > 0 {
        dex.prototypes = read_proto_ids(buf, header.proto_ids_off, header.proto_ids_size)?;
    }

    if header.field_ids_size > 0 {
        dex.fields = read_field_ids(buf, header.field_ids_off, header.field_ids_size)?;
    }

    if header.method_ids_size > 0 {
        dex.methods = read_method_ids(buf, header.method_ids_off, header.method_ids_size)?;
    }

    if header.class_defs_size > 0 {
        if lazy {
            let (classes, offsets) = super::class::read_class_defs_lazy(
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

    let map_off = header.map_off as usize;
    let map_size = u32_at(buf, map_off)? as usize;

    let mut call_site_off: Option<(u32, u32)> = None;
    let mut method_handle_off: Option<(u32, u32)> = None;
    let mut hidden_api_off: Option<(u32, u32)> = None;

    for i in 0..map_size {
        let entry = map_off + 4 + i * 12;
        let type_code = u16_at(buf, entry)?;
        let size = u32_at(buf, entry + 4)?;
        let offset = u32_at(buf, entry + 8)?;

        match type_code {
            0x0007 => call_site_off = Some((offset, size)),
            0x0008 => method_handle_off = Some((offset, size)),
            0xF000 => hidden_api_off = Some((offset, size)),
            _ => {}
        }
    }

    if header.version.supports_call_sites() {
        read_method_handles(buf, method_handle_off, &mut dex)?;
        read_call_sites(buf, call_site_off, &mut dex)?;
    }

    if header.version.supports_hidden_api() {
        if let Some((off, _)) = hidden_api_off {
            dex.hidden_api = Some(read_hidden_api(buf, off as usize, &dex)?);
        }
    }

    dex.build_lookups();
    debug!(
        version = ?dex.header.version,
        string_count = dex.strings.len(),
        type_count = dex.types.len(),
        method_count = dex.methods.len(),
        class_count = dex.classes.len(),
        "parsed DEX file"
    );
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
            let handle_type_raw = u16_at(buf, base)?;
            let member_id = u16_at(buf, base + 4)?;

            let handle_type = MethodHandleType::from_u16(handle_type_raw)
                .ok_or_else(|| invalid_method_handle_type(handle_type_raw))?;

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
            let cs_off = u32_at(buf, off as usize + i * 4)?;
            let (values, _) = read_encoded_array(buf, cs_off as usize)?;

            if values.len() < 3 {
                return Err(invalid_call_site(
                    i as u32,
                    format!("expected at least 3 elements, got {}", values.len()),
                ));
            }

            let bootstrap_method = match &values[0] {
                EncodedValue::MethodHandle(idx) => *idx,
                _ => {
                    return Err(invalid_call_site(
                        i as u32,
                        "bootstrap method is not a method handle",
                    ));
                }
            };
            let method_name = match &values[1] {
                EncodedValue::String(idx) => *idx,
                _ => {
                    return Err(invalid_call_site(i as u32, "method name is not a string"));
                }
            };
            let method_type = match &values[2] {
                EncodedValue::MethodType(idx) => *idx,
                _ => {
                    return Err(invalid_call_site(
                        i as u32,
                        "method type is not a method type",
                    ));
                }
            };

            let extra_arguments = values[3..].to_vec();

            dex.call_sites
                .push(crate::types::method_handle::CallSiteItem {
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

    let mut data_offsets = Vec::with_capacity(class_count);
    for i in 0..class_count {
        data_offsets.push(u32_at(buf, off + i * 4)?);
    }

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
                flags.push(HiddenApiFlag::from_u32(v).ok_or_else(|| invalid_hidden_api_flag(v))?);
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
    use crate::file::DexFile;
    use crate::types::access_flags::AccessFlags;
    use crate::types::class::{ClassData, ClassDef, EncodedField};
    use crate::types::encoded_value::EncodedValue;
    use crate::types::header::{DexHeader, DexVersion};
    use crate::types::method_handle::MethodHandleIdx;
    use crate::types::StringIdx;
    use crate::types::TypeIdx;
    use crate::write::encoded_value::write_encoded_array;
    use crate::DexError;

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
            container_size: 0,
            header_offset: 0,
        }
    }

    #[test]
    fn read_call_sites_rejects_short_entries() {
        let mut buf = vec![0; 4];
        buf[..4].copy_from_slice(&4u32.to_le_bytes());
        write_encoded_array(
            &mut buf,
            &[
                EncodedValue::MethodHandle(MethodHandleIdx(0)),
                EncodedValue::String(StringIdx(0)),
            ],
        );

        let mut dex = DexFile::new(empty_header());
        let error = read_call_sites(&buf, Some((0, 1)), &mut dex).unwrap_err();

        assert!(matches!(
            error,
            DexError::Malformed {
                section: "call site",
                ..
            }
        ));
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
                    field: crate::types::FieldIdx(0),
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
            crate::error::invalid_hidden_api_flag(99).to_string()
        );
    }
}
