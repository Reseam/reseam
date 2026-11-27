// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.content.Context
import app.reseam.runtime.ui.Brand
import app.reseam.runtime.ui.Theme
import app.reseam.runtime.ui.Ui
import app.reseam.runtime.ui.withRtlSupport
import java.io.IOException
import java.util.concurrent.CopyOnWriteArraySet
import org.json.JSONException
import org.json.JSONObject

class ReseamBundle internal constructor(app: Context, @JvmField val name: String) {
    internal val schema: SettingsSchema
    internal val brand: Brand

    init {
        val directory = "reseam/$name"
        try {
            val text = readSchema(app, "$directory/settings.json")
            val json = JSONObject(text)
            schema = SettingsSchema(json)
            brand = Brand(json.getJSONObject("brand"), app, directory)
        } catch (e: IOException) {
            throw IllegalArgumentException("The app has no readable settings for bundle $name", e)
        } catch (e: JSONException) {
            throw IllegalArgumentException("The app has no readable settings for bundle $name", e)
        }
    }

    val settings = Settings(app, name, schema)

    val version: String?
        get() = schema.version

    /** Builds views in the bundle's brand and the current theme of Reseam's screens. */
    fun ui(context: Context): Ui = Ui(context.withRtlSupport(), Theme.of(brand, context))

    /** Opens settings over the current screen. Callable from any thread. */
    fun openSettings() = openSettingsPanel(this)

    internal val screens = CopyOnWriteArraySet<() -> Unit>()

    /**
     * Has open settings screens read again what patch code computes for their rows, for a summary
     * that changes while no setting does, such as one measured in the background. Call it on the
     * main thread.
     */
    fun refreshRows() = screens.forEach { it() }
}

/**
 * The patched app's asset at [path]. Only in a JVM test of an extension does the app have no
 * assets; there the Reseam Gradle plugin puts the schema of the bundle's settings on the test
 * classpath instead.
 */
private fun readSchema(app: Context, path: String): String =
    (app.assets?.open(path)
            ?: ReseamBundle::class.java.classLoader?.getResourceAsStream(path)
            ?: throw IOException("No $path"))
        .bufferedReader(Charsets.UTF_8)
        .use { it.readText() }
