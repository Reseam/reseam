use crate::error::{HermesError, Result, invalid};
use crate::model::{FunctionHeader, FunctionId, HermesFile, Section};

pub(crate) fn slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid(offset, "range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| invalid(offset, "truncated section"))
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let data = slice(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

pub(crate) const fn align(offset: usize) -> usize {
    (offset + 3) & !3
}

impl<'a> HermesFile<'a> {
    /// Parses without copying file data. All structured sections, string
    /// ranges, function bodies, and exception tables are bounds checked.
    /// Unsupported versions and delta-form files return errors.
    #[expect(
        clippy::too_many_lines,
        reason = "validation follows the ordered v98 file layout"
    )]
    pub fn parse(source: &'a [u8]) -> Result<Self> {
        if slice(source, 0, 8)? != 0x1f19_03c1_03bc_1fc6_u64.to_le_bytes() {
            return Err(HermesError::Magic);
        }
        let version = read_u32(source, 8)?;
        if version != 98 {
            return Err(HermesError::Version(version));
        }
        slice(source, 0, 128)?;
        let mut header = [0; 23];
        header[0] = version;
        for (index, field) in header.iter_mut().enumerate().skip(1) {
            *field = read_u32(source, 28 + index * 4)?;
        }
        let length = header[1] as usize;
        if length < 148 || length > source.len() {
            return Err(invalid(32, "invalid file length"));
        }
        if header[3] == 0 || header[2] >= header[3] {
            return Err(invalid(36, "invalid global function"));
        }
        let sizes = [
            (header[3], 12),
            (header[4], 4),
            (header[5], 4),
            (header[6], 4),
            (header[7], 8),
            (header[8], 1),
            (header[13], 1),
            (header[14], 1),
            (header[15], 8),
            (header[9], 8),
            (header[10], 1),
            (header[11], 8),
            (header[12], 1),
            (header[18], 8),
            (header[19], 8),
        ];
        let mut cursor = 128;
        let mut sections = std::array::from_fn(|_| 0..0);
        for (section, (count, stride)) in sections.iter_mut().zip(sizes) {
            cursor = align(cursor);
            let size = (count as usize)
                .checked_mul(stride)
                .ok_or_else(|| invalid(cursor, "section size overflow"))?;
            slice(&source[..length - 20], cursor, size)?;
            *section = cursor..cursor + size;
            cursor += size;
        }
        let file = Self {
            source,
            header,
            sections,
        };
        if (file.header[20] as usize) < cursor || file.header[20] as usize > length - 20 {
            return Err(invalid(108, "invalid debug info offset"));
        }
        let mut count = 0_u64;
        let mut identifiers = 0_u64;
        for entry in file.section(Section::Kinds).as_chunks::<4>().0 {
            let value = u32::from_le_bytes(*entry);
            let run = value & 0x7fff_ffff;
            if run == 0 {
                return Err(invalid(file.sections[1].start, "empty string kind run"));
            }
            count += u64::from(run);
            if value >> 31 != 0 {
                identifiers += u64::from(run);
            }
        }
        if count != u64::from(header[6]) || identifiers != u64::from(header[5]) {
            return Err(invalid(
                file.sections[1].start,
                "string kind counts disagree",
            ));
        }
        for id in 0..file.string_count() {
            file.string(crate::StringId(id))?;
        }
        for id in 0..file.function_count() {
            let function = file.function(FunctionId(id))?;
            let h = function.header;
            if h.offset as usize + h.size as usize > file.header[20] as usize {
                return Err(invalid(
                    h.offset as usize,
                    "function overlaps debug section",
                ));
            }
            if h.flags & 8 != 0 {
                let count = read_u32(source, h.info_offset)? as usize;
                let table = slice(
                    source,
                    h.info_offset + 4,
                    count
                        .checked_mul(12)
                        .ok_or_else(|| invalid(h.info_offset, "exception table overflow"))?,
                )?;
                for entry in table.as_chunks::<12>().0 {
                    let start = read_u32(entry, 0)?;
                    let end = read_u32(entry, 4)?;
                    let target = read_u32(entry, 8)?;
                    if start > end || end > h.size || target >= h.size {
                        return Err(invalid(h.info_offset, "exception handler outside function"));
                    }
                }
            }
        }
        Ok(file)
    }
}

pub(crate) fn function_header(bytes: &[u8], offset: usize) -> Result<FunctionHeader> {
    let small = slice(bytes, offset, 12)?;
    let w1 = read_u32(small, 0)?;
    let w2 = read_u32(small, 4)?;
    if small[11] & 32 != 0 {
        let large = ((w2 >> 14) & 255) << 24 | (w1 & 0x00ff_ffff);
        let data = slice(bytes, large as usize, 37)?;
        Ok(FunctionHeader {
            offset: read_u32(data, 0)?,
            parameters: read_u32(data, 4)?,
            loop_depth: read_u32(data, 8)?,
            size: read_u32(data, 12)?,
            name: read_u32(data, 16)?,
            number_regs: read_u32(data, 20)?,
            non_pointer_regs: read_u32(data, 24)?,
            frame_size: read_u32(data, 28)?,
            read_cache: data[32],
            write_cache: data[33],
            object_cache: data[34],
            private_cache: data[35],
            flags: data[36],
            info_offset: align(large as usize + 37),
        })
    } else {
        Ok(FunctionHeader {
            offset: w1 & 0x01ff_ffff,
            parameters: (w1 >> 25) & 31,
            loop_depth: w1 >> 30,
            size: w2 & 0x3fff,
            name: (w2 >> 14) & 255,
            number_regs: (w2 >> 22) & 31,
            non_pointer_regs: w2 >> 27,
            frame_size: u32::from(small[8]),
            read_cache: small[9],
            write_cache: small[10] & 63,
            object_cache: small[10] >> 6 & 1,
            private_cache: small[10] >> 7,
            flags: small[11],
            info_offset: 0,
        })
    }
}
