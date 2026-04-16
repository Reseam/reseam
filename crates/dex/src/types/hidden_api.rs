// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Clone)]
pub struct HiddenApiData {
    pub class_flags: Vec<Option<ClassHiddenApiFlags>>,
}

#[derive(Debug, Clone)]
pub struct ClassHiddenApiFlags {
    pub static_field_flags: Vec<HiddenApiFlag>,
    pub instance_field_flags: Vec<HiddenApiFlag>,
    pub direct_method_flags: Vec<HiddenApiFlag>,
    pub virtual_method_flags: Vec<HiddenApiFlag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum HiddenApiFlag {
    Sdk = 0,
    Greylist = 1,
    Blacklist = 2,
    GreylistMaxO = 3,
    GreylistMaxP = 4,
    GreylistMaxQ = 5,
    GreylistMaxR = 6,
}

impl HiddenApiFlag {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Sdk),
            1 => Some(Self::Greylist),
            2 => Some(Self::Blacklist),
            3 => Some(Self::GreylistMaxO),
            4 => Some(Self::GreylistMaxP),
            5 => Some(Self::GreylistMaxQ),
            6 => Some(Self::GreylistMaxR),
            _ => None,
        }
    }
}
