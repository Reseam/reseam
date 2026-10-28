// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.graphics.Rect
import android.view.MotionEvent
import android.view.View
import android.widget.LinearLayout
import app.reseam.runtime.ui.SearchBar
import app.reseam.runtime.ui.Ui

/** A settings page that opens without focusing its search bar and dismisses it on outside taps. */
internal class SettingsPageLayout(ui: Ui) : LinearLayout(ui.context) {
    init {
        orientation = VERTICAL
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
