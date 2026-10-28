// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import app.reseam.runtime.RuntimeStrings

class TopBar internal constructor(private val ui: Ui, title: CharSequence?, back: Runnable) :
    LinearLayout(ui.context) {
    init {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        minimumHeight = ui.px(Tokens.TOP_BAR)
        setPaddingRelative(ui.px(Tokens.SPACE_MD), 0, ui.px(Tokens.SPACE_LG), 0)
        addView(
            ui.iconButton(Icons.ARROW_LEFT, ui.colors.onSurface, RuntimeStrings.back) {
                back.run()
            },
            LayoutParams(ui.px(Tokens.TOUCH_TARGET), ui.px(Tokens.TOUCH_TARGET)),
        )
        val label =
            ui.text(title, ui.type.titleLarge, ui.colors.onSurface).apply {
                setSingleLine(true)
                ellipsize = TextUtils.TruncateAt.END
            }
        addView(
            label,
            LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f).apply {
                marginStart = ui.px(Tokens.SPACE_SM)
            },
        )
    }

    /** Adds an icon button at the end, mirroring the back button. Returns this bar. */
    fun action(
        icon: VectorImage,
        description: CharSequence?,
        onClick: View.OnClickListener?,
    ): TopBar {
        setPaddingRelative(paddingStart, 0, ui.px(Tokens.SPACE_MD), 0)
        addView(
            ui.iconButton(icon, ui.colors.onSurface, description, onClick),
            LayoutParams(ui.px(Tokens.TOUCH_TARGET), ui.px(Tokens.TOUCH_TARGET)),
        )
        return this
    }
}
