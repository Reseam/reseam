// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.content.Context
import app.reseam.runtime.ui.Brand
import app.reseam.runtime.ui.Theme
import app.reseam.runtime.ui.Ui
import java.io.IOException
import org.json.JSONException
import org.json.JSONObject

class ReseamBundle internal constructor(app: Context, @JvmField val name: String) {
    internal val schema: SettingsSchema
    internal val brand: Brand

    init {
        val directory = "reseam/$name"
        try {
            val text =
                app.assets.open("$directory/settings.json").bufferedReader(Charsets.UTF_8).use {
                    it.readText()
                }
            val json = JSONObject(text)
            schema = SettingsSchema(json)
            brand = Brand(json.getJSONObject("brand"), app.assets, directory)
        } catch (e: IOException) {
            throw IllegalArgumentException("The app has no readable settings for bundle $name", e)
        } catch (e: JSONException) {
            throw IllegalArgumentException("The app has no readable settings for bundle $name", e)
        }
    }

    val settings = Settings(app, name, schema)

    /** Uses the system theme, or the host's declared light/dark theme before Android 10. */
    fun ui(context: Context): Ui = Ui(context, Theme.of(brand, context))

    /** Opens settings over the current screen. Callable from any thread. */
    fun openSettings() = openSettingsPanel(this)
}
