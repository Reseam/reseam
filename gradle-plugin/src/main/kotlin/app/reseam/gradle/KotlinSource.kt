// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

/** `hide_shorts_button` → `hideShortsButton`, escaped where Kotlin reserves the word. */
internal fun kotlinName(name: String): String {
    val camel =
        name
            .split('_')
            .filter(String::isNotEmpty)
            .mapIndexed { i, part ->
                if (i == 0) part else part.replaceFirstChar(Char::uppercaseChar)
            }
            .joinToString("")
    return if (camel in KOTLIN_KEYWORDS) "`$camel`" else camel
}

internal fun kotlinString(value: String): String = buildString {
    append('"')
    for (c in value) {
        when (c) {
            '"' -> append("\\\"")
            '\\' -> append("\\\\")
            '$' -> append("\\$")
            '\n' -> append("\\n")
            '\r' -> append("\\r")
            '\t' -> append("\\t")
            else -> append(c)
        }
    }
    append('"')
}

internal class KotlinWriter {
    private val out = StringBuilder()
    private var indent = 0

    fun line(text: String = "") {
        if (text.isNotEmpty()) repeat(indent) { out.append("    ") }
        out.append(text).append('\n')
    }

    fun block(header: String, body: KotlinWriter.() -> Unit) {
        line("$header {")
        indent++
        body()
        indent--
        line("}")
    }

    override fun toString() = out.toString()
}

private val KOTLIN_KEYWORDS =
    setOf(
        "as",
        "break",
        "class",
        "continue",
        "do",
        "else",
        "false",
        "for",
        "fun",
        "if",
        "in",
        "interface",
        "is",
        "null",
        "object",
        "package",
        "return",
        "super",
        "this",
        "throw",
        "true",
        "try",
        "typealias",
        "typeof",
        "val",
        "var",
        "when",
        "while",
    )

internal val RESERVED_NAMES = setOf("id", "table")
