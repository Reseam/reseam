// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Result, bail};
use boltffi_backend::bridge::jni::{
    JniType, NativeMethod, NativeParameter, NativeParameterKind, NativeReturn,
};

/// Pure functions that run outside a patch phase, so no phase check wraps them.
const UNCHECKED: [&str; 4] = [
    "_check_call",
    "_invoke_scratch_words",
    "_lower_instruction",
    "_lower_instructions",
];

/// One JVM value in a native method descriptor.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Jni {
    Void,
    Scalar(JniType),
    Array(JniType),
    ByteBuffer,
}

impl Jni {
    pub const LONG: Self = Self::Scalar(JniType::Long);

    pub fn signature(self) -> &'static str {
        match self {
            Self::Void => "V",
            Self::Scalar(ty) => ty.signature(),
            Self::Array(ty) => ty.array_signature(),
            Self::ByteBuffer => "Ljava/nio/ByteBuffer;",
        }
    }

    pub fn c_type(self) -> String {
        match self {
            Self::Void => "void".into(),
            Self::Scalar(ty) => ty.as_type_fragment().to_string(),
            Self::Array(ty) => ty.as_array_type_fragment().to_string(),
            Self::ByteBuffer => "jobject".into(),
        }
    }

    /// `LiveConnect` converts scalar Java longs through JavaScript Number, so
    /// the browser bridge passes them as one-element long arrays instead.
    pub fn transported(self) -> Self {
        if self == Self::LONG {
            Self::Array(JniType::Long)
        } else {
            self
        }
    }
}

/// The JVM values one native parameter expands to.
pub fn parameter(kind: &NativeParameterKind) -> Result<Vec<Jni>> {
    Ok(match kind {
        NativeParameterKind::Scalar(value) => vec![Jni::Scalar(value.ty())],
        NativeParameterKind::Bytes(_) => vec![Jni::ByteBuffer, Jni::Scalar(JniType::Int)],
        NativeParameterKind::Record(_) => vec![Jni::ByteBuffer],
        NativeParameterKind::DirectVector(value) => vec![Jni::Array(value.jni_type())],
        other => bail!("unsupported hosted JNI parameter: {other:?}"),
    })
}

pub fn parameters(method: &NativeMethod) -> Result<Vec<Jni>> {
    method
        .parameters()
        .iter()
        .map(NativeParameter::kind)
        .map(parameter)
        .collect::<Result<Vec<_>>>()
        .map(|slots| slots.into_iter().flatten().collect())
}

pub fn returns(value: &NativeReturn) -> Result<Jni> {
    Ok(match value {
        NativeReturn::Void | NativeReturn::Status | NativeReturn::EncodedError => Jni::Void,
        NativeReturn::Value(value) => Jni::Scalar(value.jni_type()),
        NativeReturn::Bytes | NativeReturn::Record(_) | NativeReturn::StatusWriteback(_) => {
            Jni::Array(JniType::Byte)
        }
        NativeReturn::StatusValue(value) => returns(value.value())?,
        NativeReturn::EncodedErrorValue(value) => returns(value.success().value())?,
        other => bail!("unsupported hosted JNI return: {other:?}"),
    })
}

pub fn is_checked(method: &NativeMethod) -> bool {
    let name = method.c_function().name();
    !UNCHECKED.iter().any(|suffix| name.ends_with(suffix))
}

pub fn descriptor(parameters: &[Jni], returns: Jni) -> String {
    let parameters = parameters
        .iter()
        .map(|slot| slot.signature())
        .collect::<String>();
    format!("({parameters}){}", returns.signature())
}
