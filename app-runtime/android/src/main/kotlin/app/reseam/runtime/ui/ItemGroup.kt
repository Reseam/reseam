// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.widget.LinearLayout

class ItemGroup internal constructor(private val ui: Ui) : LinearLayout(ui.context) {
    init {
        orientation = VERTICAL
    }

    fun add(item: ListItem): ItemGroup = apply {
        addView(
            item,
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply {
                if (childCount > 0) topMargin = ui.px(Tokens.SPACE_XS)
            },
        )
        (0 until childCount).forEach { index ->
            (getChildAt(index) as ListItem).setShape(index == 0, index == childCount - 1)
        }
    }
}
