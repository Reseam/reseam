// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.gradle.api.GradleException
import org.w3c.dom.Element
import org.w3c.dom.Node

internal enum class PlaceholderType(val kotlin: String) {
    TEXT("String"),
    INTEGER("Int"),
    DECIMAL("Double"),
}

internal data class Placeholder(val position: Int, val id: String, val type: PlaceholderType)

internal sealed interface StringValue {
    val placeholders: List<Placeholder>
}

internal data class PlainString(val text: String, override val placeholders: List<Placeholder>) :
    StringValue

internal data class PluralString(
    val forms: Map<String, String>,
    override val placeholders: List<Placeholder>,
) : StringValue

/** One `values[-qualifier]/strings.xml`; [locale] is a BCP 47 tag, or `default` for `values`. */
internal class StringFile(val locale: String, val strings: Map<String, StringValue>)

internal const val DEFAULT_LOCALE = "default"

private val NAME = Regex("[a-z][a-z0-9_]*")
private val QUANTITIES = setOf("zero", "one", "two", "few", "many", "other")

/**
 * Reads the `strings.xml` of every `values` directory under [root], checking translations against
 * the default.
 */
internal fun readStringFiles(root: File): List<StringFile> {
    val files =
        root
            .listFiles { file -> file.isDirectory && file.name.startsWith("values") }
            .orEmpty()
            .mapNotNull { dir -> dir.resolve("strings.xml").takeIf(File::isFile) }
            .map { StringFile(locale(it.parentFile.name), parse(it)) }
            .sortedBy { it.locale }
    val base =
        files.singleOrNull { it.locale == DEFAULT_LOCALE }
            ?: throw GradleException("$root has no values/strings.xml")
    files.filter { it !== base }.forEach { translation -> check(base, translation, root) }
    return files
}

private fun check(base: StringFile, translation: StringFile, root: File) {
    for ((name, value) in translation.strings) {
        val original =
            base.strings[name]
                ?: throw GradleException(
                    "$root: ${translation.locale} translates unknown string '$name'"
                )
        if (
            original::class != value::class ||
                original.placeholders.toSet() != value.placeholders.toSet()
        )
            throw GradleException(
                "$root: ${translation.locale} '$name' has other placeholders or form than the default"
            )
    }
}

/** `values` is the default; `values-de`, `values-pt-rBR` and `values-b+sr+Latn` name locales. */
private fun locale(directory: String): String {
    val qualifier = directory.removePrefix("values").removePrefix("-")
    return when {
        qualifier.isEmpty() -> DEFAULT_LOCALE
        qualifier.startsWith("b+") -> qualifier.removePrefix("b+").replace('+', '-')
        else -> qualifier.replace("-r", "-")
    }
}

private fun parse(file: File): Map<String, StringValue> {
    val factory = DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }
    val root = factory.newDocumentBuilder().parse(file).documentElement
    val strings = linkedMapOf<String, StringValue>()
    for (element in root.childElements()) {
        val name = element.getAttribute("name")
        if (!NAME.matches(name)) throw GradleException("$file: invalid string name '$name'")
        val value =
            when (element.tagName) {
                "string" -> plain(element, file)
                "plurals" -> plural(element, file)
                else -> throw GradleException("$file: unsupported resource <${element.tagName}>")
            }
        if (strings.put(name, value) != null)
            throw GradleException("$file: duplicate string '$name'")
    }
    return strings
}

private fun plain(element: Element, file: File): PlainString {
    val (text, placeholders) = content(element, file)
    return PlainString(text, placeholders)
}

private fun plural(element: Element, file: File): PluralString {
    val items =
        element.childElements().associate { item ->
            val quantity = item.getAttribute("quantity")
            if (item.tagName != "item" || quantity !in QUANTITIES)
                throw GradleException(
                    "$file: invalid plural item in '${element.getAttribute("name")}'"
                )
            quantity to content(item, file)
        }
    val name = element.getAttribute("name")
    val other = items["other"] ?: throw GradleException("$file: plural '$name' has no 'other' item")
    if (items.values.any { it.second.toSet() != other.second.toSet() })
        throw GradleException("$file: plural '$name' items differ in placeholders")
    if (other.second.none { it.id == "count" && it.type == PlaceholderType.INTEGER })
        throw GradleException("$file: plural '$name' needs an integer placeholder with id 'count'")
    return PluralString(items.mapValues { it.value.first }, other.second)
}

internal fun Element.childElements(): List<Element> =
    childNodes.asList().filterIsInstance<Element>()

internal fun org.w3c.dom.NodeList.asList(): List<Node> = List(length, ::item)
