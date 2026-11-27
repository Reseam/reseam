// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.content.Context
import android.content.SharedPreferences
import java.util.concurrent.CopyOnWriteArrayList

/** Stored settings isolated by bundle. Obtain them through Reseam.bundle(name).settings. */
class Settings
internal constructor(app: Context, bundle: String, internal val schema: SettingsSchema) {
    private val prefs = app.getSharedPreferences("reseam.$bundle", Context.MODE_PRIVATE)
    private val listeners = CopyOnWriteArrayList<OnChangeListener>()
    // Preferences hold listeners weakly; retain the dispatcher for the lifetime of these settings.
    private val dispatch = SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
        listeners.forEach { it.onChanged(key) }
    }

    init {
        prefs.registerOnSharedPreferenceChangeListener(dispatch)
    }

    fun interface OnChangeListener {
        /** Null indicates that every value was cleared. */
        fun onChanged(key: String?)
    }

    /** Calls the listener on the main thread after each change until removed. */
    fun addOnChangeListener(listener: OnChangeListener) {
        listeners.add(listener)
    }

    fun removeOnChangeListener(listener: OnChangeListener) {
        listeners.remove(listener)
    }

    // Each covered toggle maps to the toggles that cover it.
    private val coveredBy: Map<String, List<SchemaSetting.Toggle>> =
        schema.settings
            .filterIsInstance<SchemaSetting.Toggle>()
            .flatMap { toggle -> toggle.covers.map { it to toggle } }
            .groupBy({ it.first }, { it.second })

    /**
     * A covered toggle reads as on while a toggle that covers it is on. A setting this app shows
     * starts at the schema's default; [defaultValue] is for one it does not show.
     */
    fun getBoolean(key: String, defaultValue: Boolean): Boolean =
        prefs.getBoolean(
            key,
            schema.settingOf<SchemaSetting.Toggle>(key)?.defaultValue ?: defaultValue,
        ) || isCovered(key)

    internal fun isCovered(key: String): Boolean =
        coveredBy[key].orEmpty().any { getBoolean(it.key, it.defaultValue) }

    internal fun coverers(key: String): List<String> = coveredBy[key].orEmpty().map { it.key }

    fun getString(key: String, defaultValue: String): String =
        stored(key) ?: schema.settingOf<SchemaSetting.Textual>(key)?.defaultValue ?: defaultValue

    /**
     * A number this app shows reads as its default while the stored one is not a number on its
     * slider; [defaultValue] is for one it does not show.
     */
    fun getNumber(key: String, defaultValue: Double): Double {
        val setting = schema.settingOf<SchemaSetting.Number>(key)
        val value = stored(key)?.let(NumberText::parse)
        return if (setting == null) value ?: defaultValue
        else value?.takeIf(SliderScale(setting.slider)::covers) ?: setting.defaultValue
    }

    /** Like [getNumber], for a start and an end that must both be on the slider. */
    fun getRange(key: String, defaultValue: NumberRange): NumberRange {
        val setting = schema.settingOf<SchemaSetting.Range>(key)
        val value = stored(key)?.let(NumberText::parseRange)
        return if (setting == null) value ?: defaultValue
        else value?.takeIf(SliderScale(setting.slider)::covers) ?: setting.defaultValue
    }

    /** An ARGB color. One this app shows without opacity always reads as opaque. */
    fun getColor(key: String, defaultValue: Int): Int {
        val setting = schema.settingOf<SchemaSetting.Color>(key)
        val value = stored(key)?.let(ColorCode::parse) ?: setting?.defaultValue ?: defaultValue
        return if (setting == null || setting.opacity) value else value or OPAQUE
    }

    private fun stored(key: String): String? = prefs.getString(key, null)

    /** A choice this app shows. Unlisted stored choices read as the schema default. */
    fun getChoice(key: String): String = choice(schema.setting(key) as SchemaSetting.Choice)

    /**
     * Like [getChoice], but a choice this app does not show reads as [defaultValue], since there is
     * no list to check a stored value against.
     */
    fun getChoice(key: String, defaultValue: String): String =
        schema.settingOf<SchemaSetting.Choice>(key)?.let(::choice) ?: defaultValue

    private fun choice(choice: SchemaSetting.Choice): String =
        choice.options[choice.index(getString(choice.key, choice.defaultValue))].value

    fun setBoolean(key: String, value: Boolean) = prefs.edit().putBoolean(key, value).apply()

    fun setString(key: String, value: String) = prefs.edit().putString(key, value).apply()

    /** Fails for a number off the slider of a setting this app shows. */
    fun setNumber(key: String, value: Double) {
        val slider = schema.settingOf<SchemaSetting.Number>(key)?.slider?.let(::SliderScale)
        require(slider?.covers(value) != false) { "$value is off the slider of $key" }
        setString(key, NumberText.format(slider?.round(value) ?: value))
    }

    /** Fails for a range off the slider of a setting this app shows. */
    fun setRange(key: String, value: NumberRange) {
        val slider = schema.settingOf<SchemaSetting.Range>(key)?.slider?.let(::SliderScale)
        require(slider?.covers(value) != false) { "$value is off the slider of $key" }
        setString(key, NumberText.format(value))
    }

    fun setColor(key: String, value: Int) =
        setString(
            key,
            ColorCode.format(value, schema.settingOf<SchemaSetting.Color>(key)?.opacity != false),
        )

    internal fun text(setting: SchemaSetting): String =
        when (setting) {
            is SchemaSetting.Toggle -> getBoolean(setting.key, setting.defaultValue).toString()
            is SchemaSetting.Choice -> getChoice(setting.key, setting.defaultValue)
            is SchemaSetting.Textual -> getString(setting.key, setting.defaultValue)
            is SchemaSetting.Number ->
                NumberText.format(getNumber(setting.key, setting.defaultValue))
            is SchemaSetting.Range -> NumberText.format(getRange(setting.key, setting.defaultValue))
            is SchemaSetting.Color ->
                ColorCode.format(getColor(setting.key, setting.defaultValue), setting.opacity)
            is SchemaSetting.Action,
            is SchemaSetting.Note -> ""
        }

    /** The stored values of the settings that take effect when the app restarts. */
    internal fun restartValues(): Map<String, *> {
        val stored = prefs.all
        return schema.settings.filter { it.restart }.associate { it.key to stored[it.key] }
    }

    /** The user's own value of a toggle, which a covering toggle overrides only for reads. */
    internal fun ownBoolean(toggle: SchemaSetting.Toggle): Boolean =
        prefs.getBoolean(toggle.key, toggle.defaultValue)

    internal fun edit(changes: SharedPreferences.Editor.() -> Unit) =
        prefs.edit().apply(changes).apply()
}

private const val OPAQUE = 0xFF000000.toInt()
