// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

internal class Json private constructor() {
    private val out = StringBuilder()
    private var first = true

    fun obj(block: Json.() -> Unit) {
        out.append('{')
        first = true
        block()
        out.append('}')
        first = false
    }

    fun <T> array(items: Iterable<T>, item: Json.(T) -> Unit) {
        out.append('[')
        items.forEachIndexed { index, value ->
            if (index > 0) out.append(',')
            item(value)
        }
        out.append(']')
        first = false
    }

    fun field(name: String, value: Json.() -> Unit) {
        if (!first) out.append(',')
        string(name)
        out.append(':')
        value()
        first = false
    }

    fun field(name: String, value: String?) = field(name) { value(value) }

    fun field(name: String, value: Number) = field(name) { out.append(value) }

    fun field(name: String, value: Boolean) = field(name) { out.append(value) }

    fun value(value: String?) {
        if (value == null) nothing() else string(value)
    }

    fun nothing() {
        out.append("null")
    }

    private fun string(value: String) {
        out.append('"')
        for (ch in value) {
            when (ch) {
                '\\' -> out.append("\\\\")
                '"' -> out.append("\\\"")
                '\n' -> out.append("\\n")
                '\r' -> out.append("\\r")
                '\t' -> out.append("\\t")
                else ->
                    if (ch.code < 0x20) out.append("\\u" + ch.code.toString(16).padStart(4, '0'))
                    else out.append(ch)
            }
        }
        out.append('"')
    }

    companion object {
        fun build(block: Json.() -> Unit): String = Json().apply(block).out.toString()
    }
}
