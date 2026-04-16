// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct AccessFlags: u32 {
        const PUBLIC       = 0x0001;
        const PRIVATE      = 0x0002;
        const PROTECTED    = 0x0004;
        const STATIC       = 0x0008;
        const FINAL        = 0x0010;
        const SYNCHRONIZED = 0x0020;
        const VOLATILE     = 0x0040;
        const BRIDGE       = 0x0040;
        const TRANSIENT    = 0x0080;
        const VARARGS      = 0x0080;
        const NATIVE       = 0x0100;
        const INTERFACE    = 0x0200;
        const ABSTRACT     = 0x0400;
        const STRICT       = 0x0800;
        const SYNTHETIC    = 0x1000;
        const ANNOTATION   = 0x2000;
        const ENUM         = 0x4000;
        const CONSTRUCTOR  = 0x10000;
        const DECLARED_SYNCHRONIZED = 0x20000;
    }
}

impl AccessFlags {
    pub fn is_volatile_field(self) -> bool {
        self.contains(Self::VOLATILE)
    }
    pub fn is_bridge_method(self) -> bool {
        self.contains(Self::BRIDGE)
    }
    pub fn is_transient_field(self) -> bool {
        self.contains(Self::TRANSIENT)
    }
    pub fn is_varargs_method(self) -> bool {
        self.contains(Self::VARARGS)
    }
}
