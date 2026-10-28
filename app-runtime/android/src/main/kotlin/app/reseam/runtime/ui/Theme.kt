// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.content.Context
import android.graphics.Typeface
import android.widget.TextView
import app.reseam.runtime.Appearance

class Theme private constructor(@JvmField val brand: Brand, @JvmField val dark: Boolean) {
    @JvmField val colors = if (dark) brand.dark else brand.light
    @JvmField val type = brand.typography
    val logo: VectorImage?
        get() = if (dark) brand.darkLogo else brand.lightLogo

    fun typeface(weight: Int): Typeface = brand.typeface(weight)

    fun style(view: TextView, style: TypeStyle) {
        style.apply(view, typeface(style.weight))
    }

    companion object {
        /** The theme Reseam's screens show over `context`'s host activity. */
        internal fun of(brand: Brand, context: Context): Theme =
            Theme(brand, Appearance.isDark(context))
    }
}
