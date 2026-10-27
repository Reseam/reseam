// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

internal class PathTokens(private val data: String) {
    private var index = 0

    fun hasMore(): Boolean {
        skipSeparators()
        return index < data.length
    }

    /** The next command letter, or null when the next token is a number that repeats one. */
    fun command(): Char? {
        skipSeparators()
        val char = data[index]
        if (!char.isLetter()) return null
        index++
        return char
    }

    fun number(): Double {
        skipSeparators()
        val start = index
        if (index < data.length && data[index] in "+-") index++
        var seenDot = false
        var seenExponent = false
        while (index < data.length) {
            val char = data[index]
            when {
                char.isDigit() -> index++
                char == '.' && !seenDot && !seenExponent -> {
                    seenDot = true
                    index++
                }
                char in "eE" && !seenExponent && index > start -> {
                    seenExponent = true
                    index++
                    if (index < data.length && data[index] in "+-") index++
                }
                else -> break
            }
        }
        return data.substring(start, index).toDoubleOrNull()
            ?: throw IllegalArgumentException("expected a number at $start in path data \"$data\"")
    }

    fun flag(): Boolean {
        skipSeparators()
        return when (data.getOrNull(index++)) {
            '0' -> false
            '1' -> true
            else ->
                throw IllegalArgumentException("expected an arc flag at ${index - 1} in \"$data\"")
        }
    }

    private fun skipSeparators() {
        while (index < data.length && (data[index].isWhitespace() || data[index] == ',')) index++
    }
}
