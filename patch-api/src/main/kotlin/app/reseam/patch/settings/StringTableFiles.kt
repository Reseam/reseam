// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

/**
 * The files the Reseam Gradle plugin compiles a [StringTable] to. Settings hosts copy them into
 * patched apps, and the engine reads them over JNI to write patch text into a bundle's catalog.
 */
internal object StringTableFiles {
    /** `default`, the source text, and the BCP 47 tag of every translation. */
    @JvmStatic
    fun locales(table: StringTable): List<String> =
        String(read(table, "locales")).lines().filter(String::isNotBlank)

    /** The JSON object of the strings [locale] has. */
    @JvmStatic fun json(table: StringTable, locale: String): ByteArray = read(table, "$locale.json")

    private fun read(table: StringTable, file: String): ByteArray =
        table::class.java.getResourceAsStream("/reseam/strings/${table.id}/$file")?.use {
            it.readBytes()
        } ?: error("${table.id} has no resource $file; was it built by the Reseam Gradle plugin?")
}
