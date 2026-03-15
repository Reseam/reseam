use crate::encoding::leb128::{read_sleb128, read_uleb128, read_uleb128p1};
use crate::error::Result;
use crate::model::debug::{DebugBytecode, DebugInfo};
use crate::model::string::StringIdx;
use crate::model::types::TypeIdx;

pub fn read_debug_info(buf: &[u8], off: u32) -> Result<DebugInfo> {
    let mut pos = off as usize;

    let (line_start, n) = read_uleb128(buf, pos)?;
    pos += n;

    let (params_size, n) = read_uleb128(buf, pos)?;
    pos += n;

    let mut parameter_names = Vec::with_capacity(params_size as usize);
    for _ in 0..params_size {
        let (name_idx, n) = read_uleb128p1(buf, pos)?;
        pos += n;
        parameter_names.push(name_idx.map(StringIdx));
    }

    let mut bytecodes = Vec::new();
    loop {
        if pos >= buf.len() {
            break;
        }
        let opcode = buf[pos];
        pos += 1;

        match opcode {
            0x00 => {
                bytecodes.push(DebugBytecode::EndSequence);
                break;
            }
            0x01 => {
                let (advance, n) = read_uleb128(buf, pos)?;
                pos += n;
                bytecodes.push(DebugBytecode::AdvancePc { advance });
            }
            0x02 => {
                let (advance, n) = read_sleb128(buf, pos)?;
                pos += n;
                bytecodes.push(DebugBytecode::AdvanceLine { advance });
            }
            0x03 => {
                let (register, n) = read_uleb128(buf, pos)?;
                pos += n;
                let (name, n) = read_uleb128p1(buf, pos)?;
                pos += n;
                let (type_, n) = read_uleb128p1(buf, pos)?;
                pos += n;
                bytecodes.push(DebugBytecode::StartLocal {
                    register,
                    name: name.map(StringIdx),
                    type_: type_.map(TypeIdx),
                });
            }
            0x04 => {
                let (register, n) = read_uleb128(buf, pos)?;
                pos += n;
                let (name, n) = read_uleb128p1(buf, pos)?;
                pos += n;
                let (type_, n) = read_uleb128p1(buf, pos)?;
                pos += n;
                let (sig, n) = read_uleb128p1(buf, pos)?;
                pos += n;
                bytecodes.push(DebugBytecode::StartLocalExtended {
                    register,
                    name: name.map(StringIdx),
                    type_: type_.map(TypeIdx),
                    signature: sig.map(StringIdx),
                });
            }
            0x05 => {
                let (register, n) = read_uleb128(buf, pos)?;
                pos += n;
                bytecodes.push(DebugBytecode::EndLocal { register });
            }
            0x06 => {
                let (register, n) = read_uleb128(buf, pos)?;
                pos += n;
                bytecodes.push(DebugBytecode::RestartLocal { register });
            }
            0x07 => bytecodes.push(DebugBytecode::SetPrologueEnd),
            0x08 => bytecodes.push(DebugBytecode::SetEpilogueBegin),
            0x09 => {
                let (name, n) = read_uleb128p1(buf, pos)?;
                pos += n;
                bytecodes.push(DebugBytecode::SetFile {
                    name: name.map(StringIdx),
                });
            }
            special @ 0x0A..=0xFF => {
                let adjusted = (special - 0x0A) as i32;
                let line_advance = (adjusted % 15) - 4;
                let pc_advance = adjusted / 15;
                bytecodes.push(DebugBytecode::SpecialAdvance {
                    line_advance,
                    pc_advance: pc_advance as u32,
                });
            }
        }
    }

    Ok(DebugInfo {
        line_start,
        parameter_names,
        bytecodes,
    })
}
