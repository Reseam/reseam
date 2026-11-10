// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

/**
 * Translated strings generated from the `strings.xml` files in a module's `src/main/strings`. It
 * names only the strings without placeholders, the ones a patch can show as they are.
 */
abstract class StringTable(val id: String, private val names: Set<String>) {
    protected fun string(name: String): StringRef = find(name) ?: error("$id has no string '$name'")

    internal fun find(name: String): StringRef? = if (name in names) StringRef(this, name) else null

    override fun toString() = id
}

/** Text a settings screen shows: translated, or [Verbatim]. */
sealed interface DisplayText

data class StringRef internal constructor(val table: StringTable, val name: String) : DisplayText {
    override fun toString() = "${table.id}.$name"
}

/** Text shown as is in every language, such as a name the user chose in a patch option. */
data class Verbatim(val text: String) : DisplayText
