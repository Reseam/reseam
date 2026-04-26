// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

object AccessFlags {
    const val PUBLIC = 0x0001
    const val PRIVATE = 0x0002
    const val PROTECTED = 0x0004
    const val STATIC = 0x0008
    const val FINAL = 0x0010
    const val SYNCHRONIZED = 0x0020
    const val VOLATILE = 0x0040
    const val BRIDGE = 0x0040
    const val TRANSIENT = 0x0080
    const val VARARGS = 0x0080
    const val NATIVE = 0x0100
    const val INTERFACE = 0x0200
    const val ABSTRACT = 0x0400
    const val STRICT = 0x0800
    const val SYNTHETIC = 0x1000
    const val ANNOTATION = 0x2000
    const val ENUM = 0x4000
    const val CONSTRUCTOR = 0x10000
    const val DECLARED_SYNCHRONIZED = 0x20000
}

fun Int.isSet(flags: Int): Boolean = flags and this != 0
fun Int.isSet(flags: UInt): Boolean = flags.toInt() and this != 0
