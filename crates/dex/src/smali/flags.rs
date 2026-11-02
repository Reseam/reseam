// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::model::access_flags::AccessFlags;

/// What a set of access flags is declared on; one bit means different things
/// on fields and methods (`volatile` and `bridge` share 0x40).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationKind {
    Class,
    Field,
    Method,
}

impl DeclarationKind {
    /// The flags smali names on this kind of declaration, in baksmali's order.
    fn keywords(self) -> &'static [(AccessFlags, &'static str)] {
        match self {
            Self::Class => &[
                (AccessFlags::PUBLIC, "public"),
                (AccessFlags::PRIVATE, "private"),
                (AccessFlags::PROTECTED, "protected"),
                (AccessFlags::STATIC, "static"),
                (AccessFlags::FINAL, "final"),
                (AccessFlags::INTERFACE, "interface"),
                (AccessFlags::ABSTRACT, "abstract"),
                (AccessFlags::SYNTHETIC, "synthetic"),
                (AccessFlags::ANNOTATION, "annotation"),
                (AccessFlags::ENUM, "enum"),
            ],
            Self::Field => &[
                (AccessFlags::PUBLIC, "public"),
                (AccessFlags::PRIVATE, "private"),
                (AccessFlags::PROTECTED, "protected"),
                (AccessFlags::STATIC, "static"),
                (AccessFlags::FINAL, "final"),
                (AccessFlags::VOLATILE, "volatile"),
                (AccessFlags::TRANSIENT, "transient"),
                (AccessFlags::SYNTHETIC, "synthetic"),
                (AccessFlags::ENUM, "enum"),
            ],
            Self::Method => &[
                (AccessFlags::PUBLIC, "public"),
                (AccessFlags::PRIVATE, "private"),
                (AccessFlags::PROTECTED, "protected"),
                (AccessFlags::STATIC, "static"),
                (AccessFlags::FINAL, "final"),
                (AccessFlags::SYNCHRONIZED, "synchronized"),
                (AccessFlags::BRIDGE, "bridge"),
                (AccessFlags::VARARGS, "varargs"),
                (AccessFlags::NATIVE, "native"),
                (AccessFlags::ABSTRACT, "abstract"),
                (AccessFlags::STRICT, "strictfp"),
                (AccessFlags::SYNTHETIC, "synthetic"),
                (AccessFlags::CONSTRUCTOR, "constructor"),
                (AccessFlags::DECLARED_SYNCHRONIZED, "declared-synchronized"),
            ],
        }
    }
}

/// The smali keywords for `flags` on `member`, space separated.
pub fn access_keywords(flags: AccessFlags, kind: DeclarationKind) -> String {
    kind.keywords()
        .iter()
        .filter(|(flag, _)| flags.contains(*flag))
        .map(|(_, keyword)| *keyword)
        .collect::<Vec<_>>()
        .join(" ")
}
