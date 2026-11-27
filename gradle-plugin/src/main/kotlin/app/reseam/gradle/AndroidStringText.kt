// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import org.gradle.api.GradleException
import org.w3c.dom.Element
import org.w3c.dom.Node

private const val XLIFF = "urn:oasis:names:tc:xliff:document:1.2"
private val SPECIFIER = Regex("""%(?:(\d+)\$)?[-#+ 0,(]*\d*(?:\.\d+)?([a-zA-Z%])""")

internal class StringContent(
    val text: String,
    val html: String?,
    val placeholders: List<Placeholder>,
)

internal fun content(element: Element, file: File): StringContent {
    val name = element.getAttribute("name")
    val raw = StringBuilder()
    val tags = mutableListOf<String>()
    val placeholders = mutableListOf<Placeholder>()
    fun read(parent: Element) {
        for (node in parent.childNodes.asList()) {
            when {
                node.nodeType == Node.TEXT_NODE || node.nodeType == Node.CDATA_SECTION_NODE ->
                    raw.append(node.nodeValue)
                node is Element && node.namespaceURI == XLIFF && node.localName == "g" -> {
                    raw.append(node.textContent)
                    placeholders += placeholder(node, placeholders.size + 1, file)
                }
                node is Element && node.namespaceURI == null && styled(node) -> {
                    tags += openTag(node)
                    raw.append(TAG)
                    read(node)
                    tags += "</${node.tagName}>"
                    raw.append(TAG)
                }
                node.nodeType == Node.COMMENT_NODE -> Unit
                else -> throw GradleException("$file: '$name' has unsupported markup")
            }
        }
    }
    read(element)
    val decoded = decode(raw.toString(), file)
    val text = decoded.replace(TAG.toString(), "").trim()
    // Text without placeholders is shown as is, so a literal % needs no escaping there.
    val specifiers =
        if (placeholders.isEmpty()) 0
        else SPECIFIER.findAll(text).count { it.groupValues[2] != "%" }
    if (specifiers != placeholders.size)
        throw GradleException("$file: '$name' has a placeholder outside xliff:g")
    if (placeholders.map { it.position }.toSet() != (1..placeholders.size).toSet())
        throw GradleException("$file: '$name' needs positions 1 to ${placeholders.size}")
    return StringContent(
        text,
        if (tags.isEmpty()) null else html(decoded, tags),
        placeholders.sortedBy { it.position },
    )
}

private fun html(decoded: String, tags: List<String>): String = buildString {
    val next = tags.iterator()
    for (c in decoded) {
        when (c) {
            TAG -> append(next.next())
            '\n' -> append("<br>")
            else -> append(escape(c.toString()))
        }
    }
}

private fun styled(element: Element): Boolean {
    val allowed = STYLES[element.tagName] ?: return false
    return element.attributes.asList().all { it.nodeName in allowed }
}

private fun openTag(element: Element): String =
    element.attributes.asList().joinToString("", "<${element.tagName}", ">") {
        " ${it.nodeName}=\"${escape(it.nodeValue)}\""
    }

private fun escape(value: String) =
    value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;")

/** Marks where a styling tag sat, through decoding, in a character no string holds. */
private const val TAG = '\uE000'

private val STYLES =
    mapOf(
        "b" to emptySet(),
        "i" to emptySet(),
        "u" to emptySet(),
        "small" to emptySet(),
        "big" to emptySet(),
        "sup" to emptySet(),
        "sub" to emptySet(),
        "font" to setOf("color"),
    )

private fun placeholder(node: Element, sequence: Int, file: File): Placeholder {
    val id = node.getAttribute("id")
    val match =
        SPECIFIER.findAll(node.textContent).singleOrNull { it.groupValues[2] != "%" }
            ?: throw GradleException("$file: xliff:g '$id' must hold exactly one placeholder")
    val type =
        when (match.groupValues[2]) {
            "s" -> PlaceholderType.TEXT
            "d" -> PlaceholderType.INTEGER
            "f",
            "e",
            "g" -> PlaceholderType.DECIMAL
            else -> throw GradleException("$file: unsupported placeholder ${match.value}")
        }
    if (!Regex("[a-z][a-zA-Z0-9_]*").matches(id))
        throw GradleException("$file: invalid xliff:g id '$id'")
    return Placeholder(match.groupValues[1].toIntOrNull() ?: sequence, id, type)
}

/** Android's resource string rules: escapes, quoted spans, and collapsed whitespace elsewhere. */
private fun decode(raw: String, file: File): String = buildString {
    var quoted = false
    var space = false
    var i = 0
    while (i < raw.length) {
        val c = raw[i++]
        when {
            c == '\\' -> {
                val escaped =
                    raw.getOrNull(i++) ?: throw GradleException("$file: trailing backslash")
                when (escaped) {
                    'n' -> append('\n')
                    't' -> append('\t')
                    'u' -> append(raw.substring(i, i + 4).toInt(16).toChar()).also { i += 4 }
                    else -> append(escaped)
                }
                space = false
            }
            c == '"' -> quoted = !quoted
            c == '\'' && !quoted -> throw GradleException("$file: unescaped apostrophe in \"$raw\"")
            c.isWhitespace() && !quoted -> {
                if (!space) append(' ')
                space = true
            }
            else -> {
                append(c)
                space = false
            }
        }
    }
}
    .trim()
