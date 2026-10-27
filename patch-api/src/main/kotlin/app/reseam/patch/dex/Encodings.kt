// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

/** The smallest `const` encoding of [value] into [dest]. */
internal fun constIntOpcode(dest: Int, value: Int): Opcode =
    when {
        dest <= 15 && value in -8..7 -> Opcode.CONST_4
        value in Short.MIN_VALUE..Short.MAX_VALUE -> Opcode.CONST_16
        value and 0xFFFF == 0 -> Opcode.CONST_HIGH16
        else -> Opcode.CONST
    }

internal fun constLongOpcode(value: Long): Opcode =
    when (value) {
        in Short.MIN_VALUE..Short.MAX_VALUE -> Opcode.CONST_WIDE_16
        in Int.MIN_VALUE..Int.MAX_VALUE -> Opcode.CONST_WIDE_32
        else -> Opcode.CONST_WIDE
    }

/** The move of a [type] value whose encoding can name both registers. */
internal fun moveOpcode(dest: Int, src: Int, type: String): Opcode {
    val kind =
        when {
            registerWordCount(type) == 2 -> Opcode.MOVE_WIDE
            isReferenceType(type) -> Opcode.MOVE_OBJECT
            else -> Opcode.MOVE
        }
    // Each kind is followed by its /from16 and /16 forms.
    val width =
        when {
            dest <= 15 && src <= 15 -> 0
            dest <= 0xFF -> 1
            else -> 2
        }
    return checkNotNull(Opcode.of(kind.value + width))
}

internal fun moveResultOpcode(type: String): Opcode =
    when {
        isReferenceType(type) -> Opcode.MOVE_RESULT_OBJECT
        registerWordCount(type) == 2 -> Opcode.MOVE_RESULT_WIDE
        else -> Opcode.MOVE_RESULT
    }

/** The member of the field access family starting at [plain] that moves a [fieldType] value. */
internal fun fieldOpcode(plain: Opcode, fieldType: String): Opcode {
    // Each family runs plain, wide, object, boolean, byte, char, short.
    val kind =
        when (fieldType) {
            "I",
            "F" -> 0
            "J",
            "D" -> 1
            "Z" -> 3
            "B" -> 4
            "C" -> 5
            "S" -> 6
            else -> 2
        }
    return checkNotNull(Opcode.of(plain.value + kind))
}
