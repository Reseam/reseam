// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.settings

/**
 * A user-facing setting shown on the bundle's settings screen. Every kind but [ActionSetting] and
 * [NoteSetting] stores a value in the patched app. Its [key] is unique within the bundle; other
 * bundles keep their own settings. Only an action or a note can lack a [title].
 */
sealed class Setting<T>(
    internal val kind: SettingKind,
    val key: String,
    internal open val property: String?,
    open val title: StringRef?,
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
    NUMBER("number", "NumberValue"),
    RANGE("range", "RangeValue"),
    COLOR("color", "ColorValue"),
    FOLDER("folder", "FolderValue"),
    FILE("file", "FileValue"),
    CHOICE("choice", "ChoiceValue"),
    ACTION("action", null),
    NOTE("note", null),
}

/**
 * While this toggle is on, every toggle in [covers] reads as on and cannot be changed. Its row
 * shows [summaryOn] or [summaryOff] for its value, and [summary] when that one is missing.
 */
class ToggleSetting
internal constructor(
    key: String,
    internal override val property: String,
    override val title: StringRef,
    summary: StringRef?,
    val summaryOn: StringRef?,
    val summaryOff: StringRef?,
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
    ) {
    /**
     * The same stored setting with another default, for a patch that decides it from its options or
     * the app.
     */
    fun copy(default: Boolean = this.default) =
        ToggleSetting(
            key,
            property,
            title,
            summary,
            summaryOn,
            summaryOff,
            default,
            icon,
            takesEffect,
            covers,
        )
}

/**
 * [multiline] lets the value hold one entry per line. The row shows the value under its title, or
 * its summary while the value is empty. Without [showValue] it shows the summary only, and the
 * editor shows the value.
 */
class TextSetting
internal constructor(
    key: String,
    internal override val property: String,
    override val title: StringRef,
    summary: StringRef?,
    default: String,
    val multiline: Boolean,
    icon: Vector?,
    takesEffect: TakesEffect,
    val showValue: Boolean,
) : Setting<String>(SettingKind.TEXT, key, property, title, summary, default, icon, takesEffect)

class FolderSetting
internal constructor(
    key: String,
    internal override val property: String,
    override val title: StringRef,
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

/**
 * A file the user picks with the system picker, stored as a `content://` URI. [mimeTypes] limits
 * what can be picked.
 */
class FileSetting
internal constructor(
    key: String,
    internal override val property: String,
    override val title: StringRef,
    summary: StringRef?,
    default: String,
    val mimeTypes: List<String>,
    icon: Vector?,
    takesEffect: TakesEffect,
) : Setting<String>(SettingKind.FILE, key, property, title, summary, default, icon, takesEffect)

class ChoiceSetting
internal constructor(
    key: String,
    internal override val property: String,
    override val title: StringRef,
    summary: StringRef?,
    default: String,
    val choices: List<Choice>,
    icon: Vector?,
    takesEffect: TakesEffect,
    val showValue: Boolean,
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
        ChoiceSetting(
            key,
            property,
            title,
            summary,
            default,
            choices,
            icon,
            takesEffect,
            showValue,
        )
}

data class Choice(val value: String, val title: DisplayText)

/** A row that runs the code a patch gives it with [onAction] when tapped. It stores nothing. */
class ActionSetting
internal constructor(
    key: String,
    internal override val property: String,
    override val title: StringRef?,
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
 * A row that only shows its summary, such as notes on the settings around it, or what patch code
 * gives it with [onSummary] or [onView]. It stores nothing. A note made while patching, with
 * [noteOf], has no [property].
 */
class NoteSetting
internal constructor(
    key: String,
    internal override val property: String?,
    override val title: StringRef?,
    summary: StringRef?,
    icon: Vector?,
) :
    Setting<Unit>(
        SettingKind.NOTE,
        key,
        property,
        title,
        summary,
        Unit,
        icon,
        TakesEffect.IMMEDIATELY,
    )
