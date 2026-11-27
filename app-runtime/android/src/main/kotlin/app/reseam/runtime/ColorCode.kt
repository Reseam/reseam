// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import java.util.Locale

internal object ColorCode {
    fun format(color: Int, opacity: Boolean): String =
        if (opacity) String.format(Locale.ROOT, "#%08X", color)
        else String.format(Locale.ROOT, "#%06X", color and RGB)

    fun parse(code: String): Int? {
        val digits = code.trim().takeIf { it.startsWith('#') }?.drop(1) ?: return null
        if (!digits.all { it in HEX }) return null
        return when (digits.length) {
            6 -> (digits.toLong(16) or OPAQUE).toInt()
            8 -> digits.toLong(16).toInt()
            else -> null
        }
    }

    private const val HEX = "0123456789abcdefABCDEF"
    private const val RGB = 0xFFFFFF
    private const val OPAQUE = 0xFF000000
}
