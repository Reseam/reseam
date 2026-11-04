// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.content.Context
import android.content.SharedPreferences
import java.util.concurrent.CopyOnWriteArrayList
import org.json.JSONObject

/** Stored settings isolated by bundle. Obtain them through Reseam.bundle(name).settings. */
class Settings
internal constructor(app: Context, bundle: String, private val schema: SettingsSchema) {
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
    private val coveredBy: Map<String, List<SettingsSchema.Toggle>> =
        schema.settings
            .filterIsInstance<SettingsSchema.Toggle>()
            .flatMap { toggle -> toggle.covers.map { it to toggle } }
            .groupBy({ it.first }, { it.second })

    /** A covered toggle reads as on while a toggle that covers it is on. */
    fun getBoolean(key: String, defaultValue: Boolean): Boolean =
        prefs.getBoolean(key, defaultValue) || isCovered(key)

    internal fun isCovered(key: String): Boolean =
        coveredBy[key].orEmpty().any { getBoolean(it.key, it.defaultValue) }

    internal fun coverers(key: String): List<String> = coveredBy[key].orEmpty().map { it.key }

    fun getString(key: String, defaultValue: String): String =
        checkNotNull(prefs.getString(key, defaultValue)) { "A non-null default always yields text" }

    /** Unlisted stored choices read as the schema default. */
    fun getChoice(key: String): String {
        val choice = schema.setting(key) as SettingsSchema.Choice
        return choice.options[choice.index(getString(key, choice.defaultValue))].value
    }

    fun setBoolean(key: String, value: Boolean) = prefs.edit().putBoolean(key, value).apply()

    fun setString(key: String, value: String) = prefs.edit().putString(key, value).apply()

    internal fun snapshot(): Map<String, *> = prefs.all.toMap()

    /** The stored value of every setting that can move to another install. */
    internal fun export(): JSONObject =
        JSONObject().apply {
            schema.settings.forEach { setting ->
                val value: Any =
                    when (setting) {
                        // A covering toggle overrides reads only; the user's own value moves.
                        is SettingsSchema.Toggle ->
                            prefs.getBoolean(setting.key, setting.defaultValue)
                        is SettingsSchema.Choice -> getChoice(setting.key)
                        is SettingsSchema.Text -> getString(setting.key, setting.defaultValue)
                        // A folder holds a permission granted to this install alone.
                        is SettingsSchema.Folder -> return@forEach
                        is SettingsSchema.Action -> return@forEach
                    }
                put(setting.key, value)
            }
        }

    /**
     * Stores the values that fit a setting of this app and keeps the rest, so an export from
     * another app of the bundle cannot reset settings it does not have. Returns how many applied.
     */
    internal fun import(values: JSONObject): Int {
        val editor = prefs.edit()
        val applied =
            schema.settings.count { setting ->
                val value = values.opt(setting.key)
                when (setting) {
                    is SettingsSchema.Toggle ->
                        (value as? Boolean)?.also { editor.putBoolean(setting.key, it) }
                    is SettingsSchema.Choice ->
                        (value as? String)
                            ?.takeIf { choice -> setting.options.any { it.value == choice } }
                            ?.also { editor.putString(setting.key, it) }
                    is SettingsSchema.Text ->
                        (value as? String)?.also { editor.putString(setting.key, it) }
                    is SettingsSchema.Folder,
                    is SettingsSchema.Action -> null
                } != null
            }
        editor.apply()
        return applied
    }
}
