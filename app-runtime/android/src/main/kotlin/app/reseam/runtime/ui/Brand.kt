// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.content.res.AssetManager
import android.graphics.Typeface
import android.os.Build
import android.util.SparseArray
import org.json.JSONObject

class Brand(json: JSONObject, private val assets: AssetManager, directory: String) {
    @JvmField val name: String = json.getString("name")
    internal val light = Colors(json.getJSONObject("light"))
    internal val dark = Colors(json.getJSONObject("dark"))
    internal val typography = Typography(json.getJSONObject("typography"))
    private val logo = json.optJSONObject("logo")
    internal val lightLogo: VectorImage? = logo?.let { VectorImage.read(it.getJSONObject("light")) }
    internal val darkLogo: VectorImage? = logo?.let { VectorImage.read(it.getJSONObject("dark")) }
    private val font: String? =
        if (json.isNull("font")) null else "$directory/${json.getString("font")}"
    private val typefaces = SparseArray<Typeface>()

    @Synchronized
    internal fun typeface(weight: Int): Typeface =
        typefaces[weight]
            ?: (font?.let {
                    Typeface.Builder(assets, it).setFontVariationSettings("'wght' $weight").build()
                } ?: systemTypeface(weight))
                .also { typefaces.put(weight, it) }
}

private fun systemTypeface(weight: Int): Typeface =
    when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.P ->
            Typeface.create(Typeface.DEFAULT, weight, false)
        // Before API 28 the system font comes in regular, medium and bold only.
        weight >= 600 -> Typeface.DEFAULT_BOLD
        weight >= 500 -> Typeface.create("sans-serif-medium", Typeface.NORMAL)
        else -> Typeface.DEFAULT
    }
