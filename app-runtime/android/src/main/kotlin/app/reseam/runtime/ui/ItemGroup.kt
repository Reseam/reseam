// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.widget.LinearLayout

class ItemGroup internal constructor(private val ui: Ui) : LinearLayout(ui.context) {
    init {
        orientation = VERTICAL
    }

    fun add(item: ListItem): ItemGroup = apply {
        val previous = getChildAt(childCount - 1) as ListItem?
        addView(
            item,
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply {
                if (previous != null) topMargin = ui.px(Tokens.SPACE_XS)
            },
        )
        previous?.setShape(first = childCount == 2, last = false)
        item.setShape(first = previous == null, last = true)
    }
}
