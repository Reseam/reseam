// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity

/**
 * A bundle setting. Reads before initialization return the default; writes require initialization.
 */
sealed class SettingValue<T : Any>(val bundle: String, val key: String, private val declared: T) {
    abstract var value: T

    /**
     * The value until the user changes it: the default a patch gave the setting, else the declared
     * one.
     */
    val default: T
        get() =
            if (Reseam.isStarted) shownDefault(Reseam.bundle(bundle).schema) ?: declared
            else declared

    internal abstract fun shownDefault(schema: SettingsSchema): T?

    fun reset() {
        value = default
    }

    protected val settings: Settings
        get() = Reseam.bundle(bundle).settings
}

class ToggleValue(bundle: String, key: String, default: Boolean) :
    SettingValue<Boolean>(bundle, key, default) {
    override fun shownDefault(schema: SettingsSchema) =
        schema.settingOf<SchemaSetting.Toggle>(key)?.defaultValue

    override var value: Boolean
        get() = Reseam.isEnabled(bundle, key, default)
        set(value) {
            settings.setBoolean(key, value)
        }
}

open class TextValue(bundle: String, key: String, default: String) :
    SettingValue<String>(bundle, key, default) {
    override fun shownDefault(schema: SettingsSchema) =
        schema.settingOf<SchemaSetting.Textual>(key)?.defaultValue

    override var value: String
        get() = Reseam.getString(bundle, key, default)
        set(value) {
            settings.setString(key, value)
        }
}

class NumberValue(bundle: String, key: String, default: Double) :
    SettingValue<Double>(bundle, key, default) {
    override fun shownDefault(schema: SettingsSchema) =
        schema.settingOf<SchemaSetting.Number>(key)?.defaultValue

    override var value: Double
        get() = if (Reseam.isStarted) settings.getNumber(key, default) else default
        set(value) {
            settings.setNumber(key, value)
        }
}

data class NumberRange(@JvmField val start: Double, @JvmField val end: Double)

class RangeValue(bundle: String, key: String, start: Double, end: Double) :
    SettingValue<NumberRange>(bundle, key, NumberRange(start, end)) {
    override fun shownDefault(schema: SettingsSchema) =
        schema.settingOf<SchemaSetting.Range>(key)?.defaultValue

    override var value: NumberRange
        get() = if (Reseam.isStarted) settings.getRange(key, default) else default
        set(value) {
            settings.setRange(key, value)
        }
}

/** An ARGB color. */
class ColorValue(bundle: String, key: String, default: Int) :
    SettingValue<Int>(bundle, key, default) {
    override fun shownDefault(schema: SettingsSchema) =
        schema.settingOf<SchemaSetting.Color>(key)?.defaultValue

    override var value: Int
        get() = if (Reseam.isStarted) settings.getColor(key, default) else default
        set(value) {
            settings.setColor(key, value)
        }
}

class FolderValue(bundle: String, key: String, default: String) : TextValue(bundle, key, default) {
    /**
     * Opens the system folder picker over [activity]. The folder the user picks becomes the value,
     * then [onPicked] runs on the main thread; it does not run when the user cancels.
     */
    fun pick(activity: Activity, onPicked: Runnable) = pickFolder(activity, bundle, key, onPicked)
}

class FileValue(bundle: String, key: String, default: String, private val mimeTypes: List<String>) :
    TextValue(bundle, key, default) {
    /**
     * Opens the system file picker over [activity]. The file the user picks becomes the value, a
     * `content://` URI that stays readable, then [onPicked] runs on the main thread. It does not
     * run when the user cancels.
     */
    fun pick(activity: Activity, onPicked: Runnable) =
        pickFile(activity, bundle, key, mimeTypes, onPicked)
}

class ChoiceValue(bundle: String, key: String, default: String) :
    SettingValue<String>(bundle, key, default) {
    override fun shownDefault(schema: SettingsSchema) =
        schema.settingOf<SchemaSetting.Textual>(key)?.defaultValue

    override var value: String
        get() = Reseam.getChoice(bundle, key, default)
        set(value) {
            settings.setString(key, value)
        }
}
