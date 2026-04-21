// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::instruction::Instruction;
use super::{FieldIdx, MethodIdx, StringIdx, TypeIdx};

impl Instruction {
    pub fn method_ref(&self) -> Option<MethodIdx> {
        match self {
            Self::InvokeVirtual { method, .. }
            | Self::InvokeSuper { method, .. }
            | Self::InvokeDirect { method, .. }
            | Self::InvokeStatic { method, .. }
            | Self::InvokeInterface { method, .. }
            | Self::InvokeVirtualRange { method, .. }
            | Self::InvokeSuperRange { method, .. }
            | Self::InvokeDirectRange { method, .. }
            | Self::InvokeStaticRange { method, .. }
            | Self::InvokeInterfaceRange { method, .. }
            | Self::InvokePolymorphic { method, .. }
            | Self::InvokePolymorphicRange { method, .. } => Some(*method),
            _ => None,
        }
    }

    pub fn field_ref(&self) -> Option<FieldIdx> {
        match self {
            Self::Iget { field, .. }
            | Self::IgetWide { field, .. }
            | Self::IgetObject { field, .. }
            | Self::IgetBoolean { field, .. }
            | Self::IgetByte { field, .. }
            | Self::IgetChar { field, .. }
            | Self::IgetShort { field, .. }
            | Self::Iput { field, .. }
            | Self::IputWide { field, .. }
            | Self::IputObject { field, .. }
            | Self::IputBoolean { field, .. }
            | Self::IputByte { field, .. }
            | Self::IputChar { field, .. }
            | Self::IputShort { field, .. }
            | Self::Sget { field, .. }
            | Self::SgetWide { field, .. }
            | Self::SgetObject { field, .. }
            | Self::SgetBoolean { field, .. }
            | Self::SgetByte { field, .. }
            | Self::SgetChar { field, .. }
            | Self::SgetShort { field, .. }
            | Self::Sput { field, .. }
            | Self::SputWide { field, .. }
            | Self::SputObject { field, .. }
            | Self::SputBoolean { field, .. }
            | Self::SputByte { field, .. }
            | Self::SputChar { field, .. }
            | Self::SputShort { field, .. } => Some(*field),
            _ => None,
        }
    }

    pub fn string_ref(&self) -> Option<StringIdx> {
        match self {
            Self::ConstString { string, .. } | Self::ConstStringJumbo { string, .. } => {
                Some(*string)
            }
            _ => None,
        }
    }

    pub fn type_ref(&self) -> Option<TypeIdx> {
        match self {
            Self::ConstClass { type_, .. }
            | Self::CheckCast { type_, .. }
            | Self::InstanceOf { type_, .. }
            | Self::NewInstance { type_, .. }
            | Self::NewArray { type_, .. }
            | Self::FilledNewArray { type_, .. }
            | Self::FilledNewArrayRange { type_, .. } => Some(*type_),
            _ => None,
        }
    }

    pub fn literal(&self) -> Option<i64> {
        match self {
            Self::Const4 { value, .. } => Some(i64::from(*value)),
            Self::Const16 { value, .. } => Some(i64::from(*value)),
            Self::Const { value, .. } => Some(i64::from(*value)),
            Self::ConstHigh16 { value, .. } => Some(i64::from(*value)),
            Self::ConstWide16 { value, .. } => Some(i64::from(*value)),
            Self::ConstWide32 { value, .. } => Some(i64::from(*value)),
            Self::ConstWide { value, .. } => Some(*value),
            Self::ConstWideHigh16 { value, .. } => Some(i64::from(*value)),
            Self::AddIntLit16 { literal, .. }
            | Self::RsubIntLit16 { literal, .. }
            | Self::MulIntLit16 { literal, .. }
            | Self::DivIntLit16 { literal, .. }
            | Self::RemIntLit16 { literal, .. }
            | Self::AndIntLit16 { literal, .. }
            | Self::OrIntLit16 { literal, .. }
            | Self::XorIntLit16 { literal, .. } => Some(i64::from(*literal)),
            Self::AddIntLit8 { literal, .. }
            | Self::RsubIntLit8 { literal, .. }
            | Self::MulIntLit8 { literal, .. }
            | Self::DivIntLit8 { literal, .. }
            | Self::RemIntLit8 { literal, .. }
            | Self::AndIntLit8 { literal, .. }
            | Self::OrIntLit8 { literal, .. }
            | Self::XorIntLit8 { literal, .. }
            | Self::ShlIntLit8 { literal, .. }
            | Self::ShrIntLit8 { literal, .. }
            | Self::UshrIntLit8 { literal, .. } => Some(i64::from(*literal)),
            _ => None,
        }
    }

    pub fn is_invoke(&self) -> bool {
        matches!(
            self,
            Self::InvokeVirtual { .. }
                | Self::InvokeSuper { .. }
                | Self::InvokeDirect { .. }
                | Self::InvokeStatic { .. }
                | Self::InvokeInterface { .. }
                | Self::InvokeVirtualRange { .. }
                | Self::InvokeSuperRange { .. }
                | Self::InvokeDirectRange { .. }
                | Self::InvokeStaticRange { .. }
                | Self::InvokeInterfaceRange { .. }
                | Self::InvokePolymorphic { .. }
                | Self::InvokePolymorphicRange { .. }
                | Self::InvokeCustom { .. }
                | Self::InvokeCustomRange { .. }
        )
    }

    pub fn is_branch(&self) -> bool {
        matches!(
            self,
            Self::Goto { .. }
                | Self::Goto16 { .. }
                | Self::Goto32 { .. }
                | Self::IfEq { .. }
                | Self::IfNe { .. }
                | Self::IfLt { .. }
                | Self::IfGe { .. }
                | Self::IfGt { .. }
                | Self::IfLe { .. }
                | Self::IfEqz { .. }
                | Self::IfNez { .. }
                | Self::IfLtz { .. }
                | Self::IfGez { .. }
                | Self::IfGtz { .. }
                | Self::IfLez { .. }
                | Self::PackedSwitch { .. }
                | Self::SparseSwitch { .. }
        )
    }

    pub fn is_return(&self) -> bool {
        matches!(
            self,
            Self::ReturnVoid
                | Self::Return { .. }
                | Self::ReturnWide { .. }
                | Self::ReturnObject { .. }
        )
    }
}
