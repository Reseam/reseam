// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.view.View
import android.widget.LinearLayout

/**
 * Buttons side by side in equal shares, or stacked at full width when a label would not fit its
 * share, as longer translations often do.
 */
class ButtonRow internal constructor(ui: Ui, buttons: List<View>) : LinearLayout(ui.context) {
    private val gap = ui.px(Tokens.SPACE_SM)

    init {
        buttons.forEach { addView(it, LayoutParams(0, LayoutParams.WRAP_CONTENT)) }
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val share = (MeasureSpec.getSize(widthMeasureSpec) - gap * (childCount - 1)) / childCount
        val natural = MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED)
        val fits =
            (0 until childCount).all { index ->
                getChildAt(index).run {
                    measure(natural, natural)
                    measuredWidth <= share
                }
            }
        orientation = if (fits) HORIZONTAL else VERTICAL
        for (index in 0 until childCount) {
            val params = getChildAt(index).layoutParams as LayoutParams
            val between = if (index > 0) gap else 0
            params.width = if (fits) 0 else LayoutParams.MATCH_PARENT
            params.weight = if (fits) 1f else 0f
            params.marginStart = if (fits) between else 0
            params.topMargin = if (fits) 0 else between
        }
        super.onMeasure(widthMeasureSpec, heightMeasureSpec)
    }
}
