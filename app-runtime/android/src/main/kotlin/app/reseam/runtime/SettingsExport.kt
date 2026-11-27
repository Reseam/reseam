// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import org.json.JSONArray
import org.json.JSONObject

internal fun Settings.export(): JSONObject =
    JSONObject().apply {
        schema.settings.forEach { setting ->
            val value: Any =
                when (setting) {
                    // A covering toggle overrides reads only; the user's own value moves.
                    is SchemaSetting.Toggle -> ownBoolean(setting)
                    is SchemaSetting.Choice -> getChoice(setting.key, setting.defaultValue)
                    is SchemaSetting.Text -> getString(setting.key, setting.defaultValue)
                    is SchemaSetting.Number -> getNumber(setting.key, setting.defaultValue)
                    is SchemaSetting.Range ->
                        getRange(setting.key, setting.defaultValue).let {
                            JSONArray().put(it.start).put(it.end)
                        }
                    is SchemaSetting.Color ->
                        ColorCode.format(
                            getColor(setting.key, setting.defaultValue),
                            setting.opacity,
                        )
                    // A folder or file holds a permission granted to this install alone.
                    is SchemaSetting.Folder,
                    is SchemaSetting.File -> return@forEach
                    is SchemaSetting.Action,
                    is SchemaSetting.Note -> return@forEach
                }
            put(setting.key, value)
        }
    }

/**
 * Stores the values that fit a setting of this app and keeps the rest, so an export from another
 * app of the bundle cannot reset settings it does not have. Returns how many applied.
 */
internal fun Settings.import(values: JSONObject): Int {
    var applied = 0
    edit {
        applied =
            schema.settings.count { setting ->
                val value = values.opt(setting.key)
                when (setting) {
                    is SchemaSetting.Toggle ->
                        (value as? Boolean)?.also { putBoolean(setting.key, it) }
                    is SchemaSetting.Choice ->
                        (value as? String)
                            ?.takeIf { choice -> setting.options.any { it.value == choice } }
                            ?.also { putString(setting.key, it) }
                    is SchemaSetting.Text -> (value as? String)?.also { putString(setting.key, it) }
                    is SchemaSetting.Number ->
                        (value as? Number)
                            ?.toDouble()
                            ?.takeIf(SliderScale(setting.slider)::covers)
                            ?.also { putString(setting.key, NumberText.format(it)) }
                    is SchemaSetting.Range ->
                        (value as? JSONArray)
                            ?.takeIf { it.length() == 2 }
                            ?.let { NumberRange(it.getDouble(0), it.getDouble(1)) }
                            ?.takeIf {
                                it.start <= it.end && SliderScale(setting.slider).covers(it)
                            }
                            ?.also { putString(setting.key, NumberText.format(it)) }
                    is SchemaSetting.Color ->
                        (value as? String)?.let(ColorCode::parse)?.also {
                            putString(setting.key, ColorCode.format(it, setting.opacity))
                        }
                    is SchemaSetting.Folder,
                    is SchemaSetting.File,
                    is SchemaSetting.Action,
                    is SchemaSetting.Note -> null
                } != null
            }
    }
    return applied
}
