// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Bundles built against v1.0 call section() through this class.
@file:JvmName("SettingsKt")

package app.reseam.patch.settings

/**
 * Declares settings as properties. A setting's title is the string named after the property in
 * snake case (`hideAds` reads `hide_ads`); its summary, when present, is `hide_ads_summary`. Keys
 * start with [keyPrefix], by default the group's name in snake case, so groups that split one set
 * of settings can keep its keys.
 */
abstract class SettingsGroup(val strings: StringTable, keyPrefix: String? = null) {
    internal val keyPrefix: String = keyPrefix ?: snakeCase(this::class.simpleName!!)
}

/**
 * A reusable destination, shown as a row on its parent page. Parents allow arbitrary nesting; lower
 * orders appear first. The rows of pages come before the page's settings, unless the settings host
 * places a page after a setting with [SettingsHostBuilder.place].
 */
data class SettingsPage(
    val id: String,
    val title: StringRef,
    val parent: SettingsPage? = null,
    val order: Int = 0,
    val summary: StringRef? = null,
    val icon: Vector? = null,
) {
    init {
        require(id.isNotBlank()) { "A settings page needs a non-blank id" }
        var ancestor = parent
        while (ancestor != null) {
            require(ancestor.id != id) { "Settings page '$id' cannot be its own ancestor" }
            ancestor = ancestor.parent
        }
    }
}

/** Settings a patch gives a settings host: shown in a [SettingsSection], or only [stored]. */
sealed interface SettingsContribution {
    val settings: List<Setting<*>>
}

/** Settings shown together under [title]. Without one they read as part of the section before. */
data class SettingsSection(
    val title: StringRef?,
    override val settings: List<Setting<*>>,
    val page: SettingsPage? = null,
) : SettingsContribution

/**
 * Settings a patch's code stores that no row shows, such as values its own dialogs change. Exports,
 * imports and the defaults patches give them still cover them.
 */
class StoredSettings internal constructor(override val settings: List<Setting<*>>) :
    SettingsContribution

/** A section on the root settings screen. */
fun section(title: StringRef, vararg settings: Setting<*>) =
    SettingsSection(title, settings.toList())

/** Contributes settings to a page shared by any number of patches. */
fun section(page: SettingsPage, title: StringRef, vararg settings: Setting<*>) =
    SettingsSection(title, settings.toList(), page)

/** Settings on [page] under no heading. */
fun section(page: SettingsPage, vararg settings: Setting<*>) =
    SettingsSection(null, settings.toList(), page)

fun stored(vararg settings: Setting<*>) = StoredSettings(settings.toList())

internal fun snakeCase(name: String): String = buildString {
    for ((i, ch) in name.withIndex()) {
        if (
            ch.isUpperCase() &&
                i > 0 &&
                (name[i - 1].isLowerCase() || name.getOrNull(i + 1)?.isLowerCase() == true)
        )
            append('_')
        append(ch.lowercaseChar())
    }
}
