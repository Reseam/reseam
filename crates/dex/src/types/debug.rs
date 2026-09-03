// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{StringIdx, TypeIdx};

#[derive(Debug, Clone, PartialEq)]
pub struct DebugInfo {
    pub line_start: u32,
    pub parameter_names: Vec<Option<StringIdx>>,
    pub bytecodes: Vec<DebugBytecode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DebugBytecode {
    EndSequence,
    AdvancePc {
        advance: u32,
    },
    AdvanceLine {
        advance: i32,
    },
    StartLocal {
        register: u32,
        name: Option<StringIdx>,
        type_: Option<TypeIdx>,
    },
    StartLocalExtended {
        register: u32,
        name: Option<StringIdx>,
        type_: Option<TypeIdx>,
        signature: Option<StringIdx>,
    },
    EndLocal {
        register: u32,
    },
    RestartLocal {
        register: u32,
    },
    SetPrologueEnd,
    SetEpilogueBegin,
    SetFile {
        name: Option<StringIdx>,
    },
    SpecialAdvance {
        line_advance: i32,
        pc_advance: u32,
    },
}
