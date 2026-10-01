// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::encoding::leb128::{write_sleb128, write_uleb128, write_uleb128p1};
use crate::types::debug::{DebugBytecode, DebugInfo};

pub fn write_debug_info(buf: &mut Vec<u8>, info: &DebugInfo) {
    write_uleb128(buf, info.line_start);
    write_uleb128(buf, info.parameter_names.len() as u32);

    for name in &info.parameter_names {
        write_uleb128p1(buf, name.map(|s| s.0));
    }

    for bc in &info.bytecodes {
        match bc {
            DebugBytecode::EndSequence => buf.push(0x00),
            DebugBytecode::AdvancePc { advance } => {
                buf.push(0x01);
                write_uleb128(buf, *advance);
            }
            DebugBytecode::AdvanceLine { advance } => {
                buf.push(0x02);
                write_sleb128(buf, *advance);
            }
            DebugBytecode::StartLocal {
                register,
                name,
                type_,
            } => {
                buf.push(0x03);
                write_uleb128(buf, *register);
                write_uleb128p1(buf, name.map(|s| s.0));
                write_uleb128p1(buf, type_.map(|t| t.0));
            }
            DebugBytecode::StartLocalExtended {
                register,
                name,
                type_,
                signature,
            } => {
                buf.push(0x04);
                write_uleb128(buf, *register);
                write_uleb128p1(buf, name.map(|s| s.0));
                write_uleb128p1(buf, type_.map(|t| t.0));
                write_uleb128p1(buf, signature.map(|s| s.0));
            }
            DebugBytecode::EndLocal { register } => {
                buf.push(0x05);
                write_uleb128(buf, *register);
            }
            DebugBytecode::RestartLocal { register } => {
                buf.push(0x06);
                write_uleb128(buf, *register);
            }
            DebugBytecode::SetPrologueEnd => buf.push(0x07),
            DebugBytecode::SetEpilogueBegin => buf.push(0x08),
            DebugBytecode::SetFile { name } => {
                buf.push(0x09);
                write_uleb128p1(buf, name.map(|s| s.0));
            }
            DebugBytecode::SpecialAdvance {
                line_advance,
                pc_advance,
            } => {
                let mut pc = *pc_advance;
                let mut line = *line_advance;
                if !(-4..=10).contains(&line) {
                    buf.push(0x02);
                    write_sleb128(buf, line);
                    line = 0;
                }
                let adjusted = i64::from(line + 4) + i64::from(pc) * 15 + 0x0a;
                if adjusted > 0xff {
                    buf.push(0x01);
                    write_uleb128(buf, pc);
                    pc = 0;
                }
                buf.push((line + 4 + pc as i32 * 15 + 0x0a) as u8);
            }
        }
    }
}
