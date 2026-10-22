// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.res.Configuration
import android.content.res.Resources
import android.graphics.Typeface
import android.os.Build
import android.util.TypedValue
import android.widget.TextView

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
        /** Uses the system night mode because hosts can force their own activity configuration. */
        @JvmStatic
        fun of(brand: Brand): Theme {
            val night =
                Resources.getSystem().configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
            return Theme(brand, night == Configuration.UI_MODE_NIGHT_YES)
        }

        internal fun of(brand: Brand, context: Context): Theme {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
                hostDarkTheme(context)?.let {
                    return Theme(brand, it)
                }
            }
            return of(brand)
        }

        // Dialogs wrap the activity in their own theme, so the activity's theme is read instead.
        private fun hostDarkTheme(context: Context): Boolean? {
            val activity =
                generateSequence(context) { (it as? ContextWrapper)?.baseContext }
                    .firstOrNull { it is Activity } ?: context
            val attribute =
                activity.resources.getIdentifier("isLightTheme", "attr", activity.packageName)
            val value = TypedValue()
            return if (
                attribute != 0 &&
                    activity.theme.resolveAttribute(attribute, value, true) &&
                    value.type == TypedValue.TYPE_INT_BOOLEAN
            ) {
                value.data == 0
            } else null
        }
    }
}
