// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.settings

import kotlin.properties.PropertyDelegateProvider
import kotlin.properties.ReadOnlyProperty

/**
 * A user-facing setting stored in the patched app and shown on the bundle's settings screen. Its
 * [key] is unique within the bundle; other bundles keep their own settings.
 */
sealed class Setting<T>(
    internal val kind: SettingKind,
    val key: String,
    internal val property: String,
    val title: StringRef,
    val summary: StringRef?,
    val default: T,
    val icon: Vector?,
) {
    override fun toString() = key
}

/** How the app runtime knows a setting: its schema tag and its value class. */
internal enum class SettingKind(val tag: String, val runtimeValue: String) {
    TOGGLE("toggle", "ToggleValue"),
    TEXT("text", "TextValue"),
    FOLDER("folder", "FolderValue"),
    CHOICE("choice", "ChoiceValue"),
}

/** While this toggle is on, every toggle in [covers] reads as on and cannot be changed. */
class ToggleSetting
internal constructor(
    key: String,
    property: String,
    title: StringRef,
    summary: StringRef?,
    default: Boolean,
    icon: Vector?,
    val covers: List<ToggleSetting>,
) : Setting<Boolean>(SettingKind.TOGGLE, key, property, title, summary, default, icon)

/** [multiline] lets the value hold one entry per line. */
class TextSetting
internal constructor(
    key: String,
    property: String,
    title: StringRef,
    summary: StringRef?,
    default: String,
    val multiline: Boolean,
    icon: Vector?,
) : Setting<String>(SettingKind.TEXT, key, property, title, summary, default, icon)

class FolderSetting
internal constructor(
    key: String,
    property: String,
    title: StringRef,
    summary: StringRef?,
    default: String,
    icon: Vector?,
) : Setting<String>(SettingKind.FOLDER, key, property, title, summary, default, icon)

class ChoiceSetting
internal constructor(
    key: String,
    property: String,
    title: StringRef,
    summary: StringRef?,
    default: String,
    val choices: List<Choice>,
    icon: Vector?,
) : Setting<String>(SettingKind.CHOICE, key, property, title, summary, default, icon) {
    init {
        require(choices.any { it.value == default }) {
            "Choice setting '$key' defaults to '$default', which is not a choice"
        }
    }

    /** The same stored setting with other choices, for a patch that extends it from its options. */
    fun copy(default: String = this.default, choices: List<Choice> = this.choices) =
        ChoiceSetting(key, property, title, summary, default, choices, icon)
}

data class Choice(val value: String, val title: DisplayText)

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
 * orders appear first.
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

data class SettingsSection(
    val title: StringRef,
    val settings: List<Setting<*>>,
    val page: SettingsPage? = null,
)

/** A section on the root settings screen. */
fun section(title: StringRef, vararg settings: Setting<*>) =
    SettingsSection(title, settings.toList())

/** Contributes settings to a page shared by any number of patches. */
fun section(page: SettingsPage, title: StringRef, vararg settings: Setting<*>) =
    SettingsSection(title, settings.toList(), page)

/**
 * Declares a setting as a property of a [SettingsGroup]: `val hideAds by toggle(default = true)`.
 * The key derives from the group and property names unless given.
 */
fun toggle(
    default: Boolean,
    key: String? = null,
    icon: Vector? = null,
    covers: List<ToggleSetting> = emptyList(),
) =
    SettingDelegate(key) {
        ToggleSetting(it.key, it.property, it.title, it.summary, default, icon, covers)
    }

fun text(default: String, multiline: Boolean = false, key: String? = null, icon: Vector? = null) =
    SettingDelegate(key) {
        TextSetting(it.key, it.property, it.title, it.summary, default, multiline, icon)
    }

fun folder(default: String, key: String? = null, icon: Vector? = null) =
    SettingDelegate(key) { FolderSetting(it.key, it.property, it.title, it.summary, default, icon) }

fun choice(default: String, choices: List<Choice>, key: String? = null, icon: Vector? = null) =
    SettingDelegate(key) {
        ChoiceSetting(it.key, it.property, it.title, it.summary, default, choices, icon)
    }

internal class SettingDeclaration(
    val key: String,
    val property: String,
    val title: StringRef,
    val summary: StringRef?,
)

class SettingDelegate<S : Setting<*>>
internal constructor(
    private val key: String?,
    private val create: (SettingDeclaration) -> S,
) : PropertyDelegateProvider<SettingsGroup, ReadOnlyProperty<SettingsGroup, S>> {
    override fun provideDelegate(
        thisRef: SettingsGroup,
        property: kotlin.reflect.KProperty<*>,
    ): ReadOnlyProperty<SettingsGroup, S> {
        val name = snakeCase(property.name)
        val strings = thisRef.strings
        val title =
            strings.find(name)
                ?: error(
                    "${thisRef::class.simpleName}.${property.name} has no title: add '$name' to $strings"
                )
        val setting =
            create(
                SettingDeclaration(
                    key ?: (thisRef.keyPrefix + "." + name),
                    property.name,
                    title,
                    strings.find(name + "_summary"),
                )
            )
        return ReadOnlyProperty { _, _ -> setting }
    }
}

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
