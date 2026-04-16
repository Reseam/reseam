// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::encoded_value::EncodedValue;
use super::{FieldIdx, MethodIdx, ProtoIdx, StringIdx};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MethodHandleIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodHandle {
    pub handle_type: MethodHandleType,
    pub member: MethodHandleMember,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodHandleType {
    StaticPut,
    StaticGet,
    InstancePut,
    InstanceGet,
    InvokeStatic,
    InvokeInstance,
    InvokeConstructor,
    InvokeDirect,
    InvokeInterface,
}

impl MethodHandleType {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x00 => Some(Self::StaticPut),
            0x01 => Some(Self::StaticGet),
            0x02 => Some(Self::InstancePut),
            0x03 => Some(Self::InstanceGet),
            0x04 => Some(Self::InvokeStatic),
            0x05 => Some(Self::InvokeInstance),
            0x06 => Some(Self::InvokeConstructor),
            0x07 => Some(Self::InvokeDirect),
            0x08 => Some(Self::InvokeInterface),
            _ => None,
        }
    }

    pub fn to_u16(self) -> u16 {
        match self {
            Self::StaticPut => 0x00,
            Self::StaticGet => 0x01,
            Self::InstancePut => 0x02,
            Self::InstanceGet => 0x03,
            Self::InvokeStatic => 0x04,
            Self::InvokeInstance => 0x05,
            Self::InvokeConstructor => 0x06,
            Self::InvokeDirect => 0x07,
            Self::InvokeInterface => 0x08,
        }
    }

    pub fn is_field(self) -> bool {
        matches!(
            self,
            Self::StaticPut | Self::StaticGet | Self::InstancePut | Self::InstanceGet
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodHandleMember {
    Field(FieldIdx),
    Method(MethodIdx),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallSiteIdx(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct CallSiteItem {
    pub bootstrap_method: MethodHandleIdx,
    pub method_name: StringIdx,
    pub method_type: ProtoIdx,
    pub extra_arguments: Vec<EncodedValue>,
}
