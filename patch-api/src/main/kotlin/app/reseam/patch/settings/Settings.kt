// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.settings

/**
 * A user-facing setting shown on the bundle's settings screen. Every kind but [ActionSetting]
 * stores a value in the patched app. Its [key] is unique within the bundle; other bundles keep
 * their own settings.
 */
sealed class Setting<T>(
    internal val kind: SettingKind,
    val key: String,
    internal val property: String,
    val title: StringRef,
    val summary: StringRef?,
    val default: T,
    val icon: Vector?,
    val takesEffect: TakesEffect,
) {
    override fun toString() = key
}

/**
 * When the app picks up a change to a setting. Closing the settings after changing one that takes
 * effect [ON_RESTART] offers to restart the app; a change to one that applies [IMMEDIATELY] does
 * not.
 */
enum class TakesEffect {
    ON_RESTART,
    IMMEDIATELY,
}

/**
 * How the app runtime knows a setting: its schema tag and its value class, which an action lacks.
 */
internal enum class SettingKind(val tag: String, val runtimeValue: String?) {
    TOGGLE("toggle", "ToggleValue"),
    TEXT("text", "TextValue"),
    FOLDER("folder", "FolderValue"),
    CHOICE("choice", "ChoiceValue"),
    ACTION("action", null),
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
    takesEffect: TakesEffect,
    val covers: List<ToggleSetting>,
) :
    Setting<Boolean>(
        SettingKind.TOGGLE,
        key,
        property,
        title,
        summary,
        default,
        icon,
        takesEffect,
    )

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
    takesEffect: TakesEffect,
) : Setting<String>(SettingKind.TEXT, key, property, title, summary, default, icon, takesEffect)

class FolderSetting
internal constructor(
    key: String,
    property: String,
    title: StringRef,
    summary: StringRef?,
    default: String,
    icon: Vector?,
    takesEffect: TakesEffect,
) :
    Setting<String>(
        SettingKind.FOLDER,
        key,
        property,
        title,
        summary,
        default,
        icon,
        takesEffect,
    )

class ChoiceSetting
internal constructor(
    key: String,
    property: String,
    title: StringRef,
    summary: StringRef?,
    default: String,
    val choices: List<Choice>,
    icon: Vector?,
    takesEffect: TakesEffect,
) :
    Setting<String>(
        SettingKind.CHOICE,
        key,
        property,
        title,
        summary,
        default,
        icon,
        takesEffect,
    ) {
    init {
        require(choices.any { it.value == default }) {
            "Choice setting '$key' defaults to '$default', which is not a choice"
        }
    }

    /** The same stored setting with other choices, for a patch that extends it from its options. */
    fun copy(default: String = this.default, choices: List<Choice> = this.choices) =
        ChoiceSetting(key, property, title, summary, default, choices, icon, takesEffect)
}

data class Choice(val value: String, val title: DisplayText)

/** A row that runs the code a patch gives it with [onAction] when tapped. It stores nothing. */
class ActionSetting
internal constructor(
    key: String,
    property: String,
    title: StringRef,
    summary: StringRef?,
    icon: Vector?,
) :
    Setting<Unit>(
        SettingKind.ACTION,
        key,
        property,
        title,
        summary,
        Unit,
        icon,
        TakesEffect.IMMEDIATELY,
    )

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
