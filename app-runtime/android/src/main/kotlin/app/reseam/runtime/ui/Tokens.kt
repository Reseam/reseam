// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.content.Context

object Tokens {
    const val SPACE_XS = 4
    const val SPACE_SM = 8
    const val SPACE_MD = 12
    const val SPACE_LG = 16
    const val SPACE_XXL = 24
    const val SPACE_XXXL = 32
    const val RADIUS_XS = 4
    const val RADIUS_SM = 8
    const val RADIUS_LG = 16
    const val RADIUS_XL = 28
    const val TOUCH_TARGET = 48
    const val ICON_SM = 18
    const val ICON_MD = 24
    const val BUTTON = 40
    const val TOP_BAR = 64
    const val SEARCH_BAR = 56
    const val LIST_ITEM = 64
    const val LOGO = 40
    const val DIALOG_MIN_WIDTH = 280
    const val DIALOG_MAX_WIDTH = 560
    const val DISABLED = 0.38f
    const val PRESSED = 0.10f
    const val SCRIM = 0.32f

    @JvmStatic
    fun px(context: Context, dp: Float): Int =
        Math.round(dp * context.resources.displayMetrics.density)
}
