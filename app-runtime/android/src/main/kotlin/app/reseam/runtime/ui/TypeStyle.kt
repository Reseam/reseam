// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.graphics.Typeface
import android.os.Build
import android.util.TypedValue
import android.widget.TextView
import org.json.JSONObject

/** A Material 3 type role: size and line height in sp, weight, and tracking in sp. */
class TypeStyle internal constructor(json: JSONObject) {
    @JvmField val size = json.getDouble("size").toFloat()
    @JvmField val lineHeight = json.getDouble("lineHeight").toFloat()
    @JvmField val weight = json.getInt("weight")
    @JvmField val tracking = json.getDouble("tracking").toFloat()

    internal fun apply(view: TextView, typeface: Typeface) {
        view.typeface = typeface
        view.setTextSize(TypedValue.COMPLEX_UNIT_SP, size)
        view.letterSpacing = tracking / size
        view.includeFontPadding = false
        val height =
            Math.round(
                TypedValue.applyDimension(
                    TypedValue.COMPLEX_UNIT_SP,
                    lineHeight,
                    view.resources.displayMetrics,
                )
            )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            view.lineHeight = height
        } else {
            view.setLineSpacing((height - view.paint.getFontMetricsInt(null)).toFloat(), 1f)
        }
    }
}
