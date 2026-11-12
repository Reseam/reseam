// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::Problem;

/// An Android ABI, named as a device's `Build.SUPPORTED_ABIS` lists it and as
/// an APK's `lib/<abi>/` folder holds its native libraries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(as = "String"))]
#[boltffi::data]
pub enum Abi {
    Arm64V8a,
    ArmeabiV7a,
    Armeabi,
    X86_64,
    X86,
    Riscv64,
    Mips64,
    Mips,
}

impl Abi {
    const ALL: [Self; 8] = [
        Self::Arm64V8a,
        Self::ArmeabiV7a,
        Self::Armeabi,
        Self::X86_64,
        Self::X86,
        Self::Riscv64,
        Self::Mips64,
        Self::Mips,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Arm64V8a => "arm64-v8a",
            Self::ArmeabiV7a => "armeabi-v7a",
            Self::Armeabi => "armeabi",
            Self::X86_64 => "x86_64",
            Self::X86 => "x86",
            Self::Riscv64 => "riscv64",
            Self::Mips64 => "mips64",
            Self::Mips => "mips",
        }
    }
}

impl std::str::FromStr for Abi {
    type Err = Problem;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|abi| abi.as_str() == value)
            .ok_or_else(|| Problem::UnknownAbi {
                value: value.to_owned(),
            })
    }
}

impl Serialize for Abi {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Abi {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

impl std::fmt::Display for Abi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
