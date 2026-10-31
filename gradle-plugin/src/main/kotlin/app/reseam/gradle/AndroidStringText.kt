// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import org.gradle.api.GradleException
import org.w3c.dom.Element
import org.w3c.dom.Node

private const val XLIFF = "urn:oasis:names:tc:xliff:document:1.2"
private val SPECIFIER = Regex("""%(?:(\d+)\$)?[-#+ 0,(]*\d*(?:\.\d+)?([a-zA-Z%])""")

/** The decoded text and its `xliff:g` placeholders, in position order. */
internal fun content(element: Element, file: File): Pair<String, List<Placeholder>> {
    val raw = StringBuilder()
    val placeholders = mutableListOf<Placeholder>()
    for (node in element.childNodes.asList()) {
        when {
            node.nodeType == Node.TEXT_NODE || node.nodeType == Node.CDATA_SECTION_NODE ->
                raw.append(node.nodeValue)
            node is Element && node.namespaceURI == XLIFF && node.localName == "g" -> {
                raw.append(node.textContent)
                placeholders += placeholder(node, placeholders.size + 1, file)
            }
            node.nodeType == Node.COMMENT_NODE -> Unit
            else ->
                throw GradleException(
                    "$file: '${element.getAttribute("name")}' has unsupported markup"
                )
        }
    }
    val text = decode(raw.toString(), file)
    // Text without placeholders is shown as is, so a literal % needs no escaping there.
    val specifiers =
        if (placeholders.isEmpty()) 0
        else SPECIFIER.findAll(text).count { it.groupValues[2] != "%" }
    if (specifiers != placeholders.size)
        throw GradleException(
            "$file: '${element.getAttribute("name")}' has a placeholder outside xliff:g"
        )
    if (placeholders.map { it.position }.toSet() != (1..placeholders.size).toSet())
        throw GradleException(
            "$file: '${element.getAttribute("name")}' needs positions 1 to ${placeholders.size}"
        )
    return text to placeholders.sortedBy { it.position }
}

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
