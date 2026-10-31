// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.graphics.Color
import org.json.JSONObject

class Colors internal constructor(json: JSONObject) {
    @JvmField val primary = Color.parseColor(json.getString("primary"))
    @JvmField val onPrimary = Color.parseColor(json.getString("onPrimary"))
    @JvmField val primaryContainer = Color.parseColor(json.getString("primaryContainer"))
    @JvmField val onPrimaryContainer = Color.parseColor(json.getString("onPrimaryContainer"))
    @JvmField val secondaryContainer = Color.parseColor(json.getString("secondaryContainer"))
    @JvmField val onSecondaryContainer = Color.parseColor(json.getString("onSecondaryContainer"))
    @JvmField val surface = Color.parseColor(json.getString("surface"))
    @JvmField val surfaceContainerLow = Color.parseColor(json.getString("surfaceContainerLow"))
    @JvmField val surfaceContainer = Color.parseColor(json.getString("surfaceContainer"))
    @JvmField val surfaceContainerHigh = Color.parseColor(json.getString("surfaceContainerHigh"))
    @JvmField
    val surfaceContainerHighest = Color.parseColor(json.getString("surfaceContainerHighest"))
    @JvmField val onSurface = Color.parseColor(json.getString("onSurface"))
    @JvmField val onSurfaceVariant = Color.parseColor(json.getString("onSurfaceVariant"))
    @JvmField val outline = Color.parseColor(json.getString("outline"))
    @JvmField val outlineVariant = Color.parseColor(json.getString("outlineVariant"))
    @JvmField val error = Color.parseColor(json.getString("error"))
    @JvmField val onError = Color.parseColor(json.getString("onError"))
    @JvmField val errorContainer = Color.parseColor(json.getString("errorContainer"))
    @JvmField val onErrorContainer = Color.parseColor(json.getString("onErrorContainer"))
    @JvmField val scrim = Color.parseColor(json.getString("scrim"))

    companion object {
        @JvmStatic
        fun withOpacity(color: Int, opacity: Float): Int =
            (color and 0x00FFFFFF) or (Math.round(Color.alpha(color) * opacity) shl 24)
    }
}
