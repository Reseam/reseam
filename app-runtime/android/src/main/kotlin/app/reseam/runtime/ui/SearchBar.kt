// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.text.InputType
import android.view.Gravity
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.LinearLayout
import app.reseam.runtime.RuntimeStrings

class SearchBar internal constructor(ui: Ui, hint: CharSequence?) : LinearLayout(ui.context) {
    private val field = EditText(ui.context)

    init {
        val colors = ui.colors
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        minimumHeight = ui.px(Tokens.SEARCH_BAR)
        setPadding(ui.px(Tokens.SPACE_LG), 0, ui.px(Tokens.SPACE_XS), 0)
        background = ui.shape(colors.surfaceContainerHigh, Tokens.SEARCH_BAR / 2f)
        addView(
            ui.icon(Icons.SEARCH, Tokens.ICON_MD, colors.onSurfaceVariant),
            LayoutParams(ui.px(Tokens.ICON_MD), ui.px(Tokens.ICON_MD)),
        )
        field.apply {
            ui.theme.style(this, ui.type.bodyLarge)
            setTextColor(colors.onSurface)
            setHintTextColor(colors.onSurfaceVariant)
            this.hint = hint
            background = null
            setSingleLine(true)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            imeOptions = EditorInfo.IME_ACTION_SEARCH
            setPadding(ui.px(Tokens.SPACE_LG), 0, 0, 0)
            setOnEditorActionListener { view, action, _ ->
                if (action != EditorInfo.IME_ACTION_SEARCH) false
                else {
                    view.clearFocus()
                    ui.context
                        .getSystemService(InputMethodManager::class.java)
                        .hideSoftInputFromWindow(view.windowToken, 0)
                    true
                }
            }
        }
        addView(field, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
        val clear =
            ui.iconButton(Icons.CLOSE, colors.onSurfaceVariant, RuntimeStrings.clearSearch) {
                field.setText("")
            }
        clear.visibility = GONE
        addView(clear, LayoutParams(ui.px(Tokens.TOUCH_TARGET), ui.px(Tokens.TOUCH_TARGET)))
        onQueryChange { clear.visibility = if (it.isEmpty()) GONE else VISIBLE }
    }

    fun interface OnQueryChangeListener {
        fun onQueryChanged(query: String)
    }

    fun onQueryChange(listener: OnQueryChangeListener) {
        field.afterTextChanged(listener::onQueryChanged)
    }
}
