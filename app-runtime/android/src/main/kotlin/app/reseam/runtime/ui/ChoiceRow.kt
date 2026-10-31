// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.view.Gravity
import android.widget.LinearLayout

internal class ChoiceRow(ui: Ui, title: CharSequence?, checked: Boolean) :
    LinearLayout(ui.context) {
    init {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        minimumHeight = ui.px(Tokens.SEARCH_BAR)
        setPadding(ui.px(Tokens.SPACE_XXL), 0, ui.px(Tokens.SPACE_XXL), 0)
        background = ui.ripple(null, 0f, 0f, ui.colors.onSurface)
        isFocusable = true
        addView(
            ui.text(title, ui.type.bodyLarge, ui.colors.onSurface),
            LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f),
        )
        if (checked)
            addView(
                ui.icon(Icons.CHECK, Tokens.ICON_MD, ui.colors.primary),
                LayoutParams(ui.px(Tokens.ICON_MD), ui.px(Tokens.ICON_MD)),
            )
        isSelected = checked
    }
}
