// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.icu.text.PluralRules
import java.io.FileNotFoundException
import java.util.Locale
import org.json.JSONObject

/**
 * Lazily reads translated assets from a table directory. Each name falls back through the current
 * app locale's full tag, language and script, language, then default. Missing names and use before
 * Reseam.init throw; malformed JSON and formatting errors propagate to the caller.
 */
class StringTable(private val directory: String) {
    operator fun get(name: String): String = lookup(name, currentLocale).getString(name)

    fun format(name: String, vararg args: Any?): String {
        val locale = currentLocale
        return String.format(locale, lookup(name, locale).getString(name), *args)
    }

    /** Uses CLDR categories for the current locale, falling back to the entry's other form. */
    fun plural(name: String, count: Int, vararg args: Any?): String {
        val locale = currentLocale
        val forms = lookup(name, locale).getJSONObject(name)
        val category = PluralRules.forLocale(locale).select(count.toDouble())
        val pattern = forms.getString(if (forms.has(category)) category else "other")
        return String.format(locale, pattern, *args)
    }

    private val currentLocale: Locale
        get() = Reseam.context.resources.configuration.locales[0]

    private fun lookup(name: String, locale: Locale): JSONObject {
        val tags = buildList {
            add(locale.toLanguageTag())
            if (locale.script.isNotEmpty()) add("${locale.language}-${locale.script}")
            add(locale.language)
            add("default")
        }
            .distinct()
        return tags.firstNotNullOfOrNull { tag ->
            loadStringTable(directory, tag)?.takeIf { it.has(name) }
        } ?: throw IllegalArgumentException("Unknown string $name in $directory")
    }
}

private data class TableKey(val directory: String, val tag: String)

private val stringTables = mutableMapOf<TableKey, JSONObject?>()

private fun loadStringTable(directory: String, tag: String): JSONObject? =
    synchronized(stringTables) {
        val assets = Reseam.context.assets
        val key = TableKey(directory, tag)
        if (key !in stringTables) {
            stringTables[key] =
                try {
                    assets.open("$directory/$tag.json").bufferedReader(Charsets.UTF_8).use {
                        JSONObject(it.readText())
                    }
                } catch (_: FileNotFoundException) {
                    null
                }
        }
        stringTables[key]
    }
