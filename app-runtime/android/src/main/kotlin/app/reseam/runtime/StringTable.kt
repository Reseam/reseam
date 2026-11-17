// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.icu.text.PluralRules
import android.icu.util.ULocale
import android.os.LocaleList
import android.text.TextUtils
import java.util.Locale
import org.json.JSONObject

/**
 * Lazily reads translated assets from a table directory. Each name comes from the first table that
 * has it in the order of the language picked in Reseam's settings, then the app's locales, the way
 * the engine resolves patch text: for each locale, tables of its language in the same script, its
 * region first, then none, then other regions. The default text is English, so an English locale
 * ends the order. Missing names and use before Reseam.init throw; malformed JSON and formatting
 * errors propagate to the caller.
 */
class StringTable(private val directory: String) {
    operator fun get(name: String): String = find(name).strings.getString(name)

    fun format(name: String, vararg args: Any?): String {
        val table = find(name)
        return String.format(table.locale, table.strings.getString(name), *args)
    }

    /** Uses CLDR categories of the text's language, falling back to the entry's other form. */
    fun plural(name: String, count: Int, vararg args: Any?): String {
        val table = find(name)
        val forms = table.strings.getJSONObject(name)
        val category = PluralRules.forLocale(table.locale).select(count.toDouble())
        val pattern = forms.getString(if (forms.has(category)) category else "other")
        return String.format(table.locale, pattern, *args)
    }

    /**
     * The layout direction, as View.LAYOUT_DIRECTION_LTR or _RTL, of the language this table shows
     * first, so a screen of its text reads the way the text does.
     */
    val layoutDirection: Int
        get() = TextUtils.getLayoutDirectionFromLocale(readingOrder().first().locale)

    /** Each language the table has text in, by the tag Appearance.language takes. */
    internal val languages: Map<String, Locale>
        get() = translations + (Locale.ENGLISH.language to Locale.ENGLISH)

    /** By the tag in each file's name, which Locale does not always give back. */
    private val translations: Map<String, Locale> by lazy {
        Reseam.context.assets
            .list(directory)
            .orEmpty()
            .map { it.removeSuffix(".json") }
            .filter { it != DEFAULT }
            .associateWith(Locale::forLanguageTag)
    }

    @Volatile private var order: Order? = null

    private class Order(val locales: LocaleList, val tables: List<LoadedTable>)

    private fun readingOrder(): List<LoadedTable> {
        val locales = readingLocales()
        val cached = order
        if (cached != null && cached.locales == locales) return cached.tables
        val tags = buildList {
            for (index in 0 until locales.size()) {
                val wanted = locales[index]
                val source = wanted.language == Locale.ENGLISH.language
                translations
                    .mapNotNull { (tag, table) -> rank(table, wanted)?.let { tag to it } }
                    .filter { (_, rank) -> !source || rank == Rank.REGION }
                    .sortedBy { (_, rank) -> rank }
                    .forEach { (tag, _) -> if (tag !in this) add(tag) }
                if (source) break
            }
        }
        val tables =
            tags.map { LoadedTable(translations.getValue(it), loadStringTable(directory, it)) } +
                LoadedTable(Locale.ENGLISH, loadStringTable(directory, DEFAULT))
        order = Order(locales, tables)
        return tables
    }

    private fun readingLocales(): LocaleList {
        val app = Reseam.context.resources.configuration.locales
        val chosen = Appearance.language ?: return app
        val locales = listOf(Locale.forLanguageTag(chosen)) + List(app.size()) { app[it] }
        return LocaleList(*locales.distinct().toTypedArray())
    }

    private fun find(name: String): LoadedTable =
        readingOrder().firstOrNull { it.strings.has(name) }
            ?: throw IllegalArgumentException("Unknown string $name in $directory")
}

private class LoadedTable(val locale: Locale, val strings: JSONObject)

private enum class Rank {
    REGION,
    NO_REGION,
    OTHER_REGION,
}

/** How [table] fits [wanted]: its language in the same likely script, or null when it does not. */
private fun rank(table: Locale, wanted: Locale): Rank? =
    when {
        table.language != wanted.language || likelyScript(table) != likelyScript(wanted) -> null
        table.country.isEmpty() -> Rank.NO_REGION
        table.country == wanted.country -> Rank.REGION
        else -> Rank.OTHER_REGION
    }

private fun likelyScript(locale: Locale): String =
    ULocale.addLikelySubtags(ULocale.forLocale(locale)).script

private const val DEFAULT = "default"

private data class TableKey(val directory: String, val tag: String)

private val stringTables = mutableMapOf<TableKey, JSONObject>()

private fun loadStringTable(directory: String, tag: String): JSONObject =
    synchronized(stringTables) {
        val key = TableKey(directory, tag)
        stringTables.getOrPut(key) {
            Reseam.context.assets.open("$directory/$tag.json").bufferedReader(Charsets.UTF_8).use {
                JSONObject(it.readText())
            }
        }
    }
