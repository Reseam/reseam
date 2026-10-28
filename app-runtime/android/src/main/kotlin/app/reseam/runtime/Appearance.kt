// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.res.Configuration
import android.content.res.Resources
import android.os.Build
import android.util.TypedValue

/**
 * Whether Reseam's screens are dark: the theme the user picked, and until they pick one, the
 * system's, or before Android 10, which has no system dark theme, the one the host activity
 * declares.
 */
internal object Appearance {
    private val prefs
        get() = Reseam.context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun isDark(context: Context): Boolean =
        if (prefs.contains(DARK)) prefs.getBoolean(DARK, false) else isDefaultDark(context)

    fun toggle(context: Context) = prefs.edit().putBoolean(DARK, !isDark(context)).apply()

    private fun isDefaultDark(context: Context): Boolean =
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) isHostDark(context) ?: false
        else isSystemDark()

    // Hosts can force their own activity configuration, so only the system's is reliable.
    private fun isSystemDark(): Boolean =
        Resources.getSystem().configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK ==
            Configuration.UI_MODE_NIGHT_YES

    // Dialogs wrap the activity in their own theme, so the activity's theme is read instead.
    private fun isHostDark(context: Context): Boolean? {
        val activity =
            generateSequence(context) { (it as? ContextWrapper)?.baseContext }
                .firstOrNull { it is Activity } ?: return null
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

private const val PREFS = "reseam"
private const val DARK = "dark_theme"
