// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::encoding::leb128::{
    read_sleb128_with_opts, read_uleb128_with_opts, read_uleb128p1_with_opts,
};
use crate::error::{Result, require_len};
use crate::types::debug::{DebugBytecode, DebugInfo};
use crate::types::header::ParseOptions;
use crate::types::{Pool, StringIdx, TypeIdx};

pub(crate) enum DebugEvent {
    LineStart(u32),
    ParameterCount,
    Parameter(Option<StringIdx>),
    Bytecode(DebugBytecode),
}

pub(crate) struct DebugIndex {
    pub pool: Pool,
    pub value: Option<u32>,
    pub range: std::ops::Range<usize>,
}

pub(crate) struct DebugRecord {
    pub event: DebugEvent,
    pub range: std::ops::Range<usize>,
    pub indices: smallvec::SmallVec<[DebugIndex; 3]>,
}

struct DebugCursor<'a> {
    buf: &'a [u8],
    opts: ParseOptions,
    pos: usize,
    indices: smallvec::SmallVec<[DebugIndex; 3]>,
}

impl DebugCursor<'_> {
    fn uleb(&mut self) -> Result<u32> {
        let (value, size) = read_uleb128_with_opts(self.buf, self.pos, self.opts)?;
        self.pos += size;
        Ok(value)
    }

    fn sleb(&mut self) -> Result<i32> {
        let (value, size) = read_sleb128_with_opts(self.buf, self.pos, self.opts)?;
        self.pos += size;
        Ok(value)
    }

    fn index(&mut self, pool: Pool) -> Result<Option<u32>> {
        let start = self.pos;
        let (value, size) = read_uleb128p1_with_opts(self.buf, self.pos, self.opts)?;
        self.pos += size;
        self.indices.push(DebugIndex {
            pool,
            value,
            range: start..self.pos,
        });
        Ok(value)
    }

    fn record(&mut self, start: usize, event: DebugEvent) -> DebugRecord {
        DebugRecord {
            event,
            range: start..self.pos,
            indices: std::mem::take(&mut self.indices),
        }
    }
}

pub(crate) fn walk_debug_info(
    buf: &[u8],
    off: u32,
    opts: ParseOptions,
    mut visit: impl FnMut(DebugRecord) -> Result<()>,
) -> Result<()> {
    use DebugBytecode::{
        AdvanceLine, AdvancePc, EndLocal, EndSequence, RestartLocal, SetEpilogueBegin, SetFile,
        SetPrologueEnd, SpecialAdvance, StartLocal, StartLocalExtended,
    };
    let mut cursor = DebugCursor {
        buf,
        opts,
        pos: off as usize,
        indices: smallvec::SmallVec::new(),
    };
    let start = cursor.pos;
    let line = cursor.uleb()?;
    visit(cursor.record(start, DebugEvent::LineStart(line)))?;
    let start = cursor.pos;
    let count = cursor.uleb()?;
    visit(cursor.record(start, DebugEvent::ParameterCount))?;
    for _ in 0..count {
        let start = cursor.pos;
        let name = cursor.index(Pool::String)?.map(StringIdx);
        visit(cursor.record(start, DebugEvent::Parameter(name)))?;
    }
    loop {
        let start = cursor.pos;
        require_len(buf, start, 1, "debug info")?;
        let opcode = buf[start];
        cursor.pos += 1;
        let bytecode = match opcode {
            0 => EndSequence,
            1 => AdvancePc {
                advance: cursor.uleb()?,
            },
            2 => AdvanceLine {
                advance: cursor.sleb()?,
            },
            3 => StartLocal {
                register: cursor.uleb()?,
                name: cursor.index(Pool::String)?.map(StringIdx),
                type_: cursor.index(Pool::Type)?.map(TypeIdx),
            },
            4 => StartLocalExtended {
                register: cursor.uleb()?,
                name: cursor.index(Pool::String)?.map(StringIdx),
                type_: cursor.index(Pool::Type)?.map(TypeIdx),
                signature: cursor.index(Pool::String)?.map(StringIdx),
            },
            5 => EndLocal {
                register: cursor.uleb()?,
            },
            6 => RestartLocal {
                register: cursor.uleb()?,
            },
            7 => SetPrologueEnd,
            8 => SetEpilogueBegin,
            9 => SetFile {
                name: cursor.index(Pool::String)?.map(StringIdx),
            },
            special => {
                let adjusted = i32::from(special - 10);
                SpecialAdvance {
                    line_advance: adjusted % 15 - 4,
                    pc_advance: (adjusted / 15) as u32,
                }
            }
        };
        visit(cursor.record(start, DebugEvent::Bytecode(bytecode)))?;
        if opcode == 0 {
            return Ok(());
        }
    }
}

pub fn read_debug_info(buf: &[u8], off: u32, opts: ParseOptions) -> Result<DebugInfo> {
    let mut info = DebugInfo {
        line_start: 0,
        parameter_names: Vec::new(),
        bytecodes: Vec::new(),
    };
    walk_debug_info(buf, off, opts, |record| {
        match record.event {
            DebugEvent::LineStart(line) => info.line_start = line,
            DebugEvent::ParameterCount => {}
            DebugEvent::Parameter(name) => info.parameter_names.push(name),
            DebugEvent::Bytecode(bytecode) => info.bytecodes.push(bytecode),
        }
        Ok(())
    })?;
    Ok(info)
}
