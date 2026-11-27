// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.graphics.Rect
import android.view.MotionEvent
import android.view.View
import android.widget.LinearLayout

internal class PageLayout(ui: Ui) : LinearLayout(ui.context) {
    init {
        orientation = VERTICAL
        // Hosts targeting API 35 draw edge to edge; keep the bar below the status bar.
        fitsSystemWindows = true
        // Gives initial focus somewhere to go other than the search editor.
        isFocusableInTouchMode = true
    }

    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        if (event.action == MotionEvent.ACTION_DOWN) {
            val searchBar =
                generateSequence(findFocus()) { it.parent as? View }.firstOrNull { it is SearchBar }
                    as SearchBar?
            val bounds = Rect()
            if (
                searchBar != null &&
                    !(searchBar.getGlobalVisibleRect(bounds) &&
                        bounds.contains(event.rawX.toInt(), event.rawY.toInt()))
            ) {
                searchBar.dismiss()
            }
        }
        return super.dispatchTouchEvent(event)
    }
}
