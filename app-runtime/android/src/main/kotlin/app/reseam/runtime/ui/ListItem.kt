// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.view.Gravity
import android.view.View
import android.widget.ImageView
import android.widget.LinearLayout

class ListItem internal constructor(private val ui: Ui, title: CharSequence?) :
    LinearLayout(ui.context) {
    private val icon =
        ImageView(ui.context).apply {
            visibility = GONE
            importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO
        }
    private val titleView = ui.text(title, ui.type.bodyLarge, ui.colors.onSurface)
    private val supportingView =
        ui.text("", ui.type.bodyMedium, ui.colors.onSurfaceVariant).apply { visibility = GONE }
    private var trailingView: View? = null

    init {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        minimumHeight = ui.px(Tokens.LIST_ITEM)
        setPadding(
            ui.px(Tokens.SPACE_LG),
            ui.px(Tokens.SPACE_MD),
            ui.px(Tokens.SPACE_LG),
            ui.px(Tokens.SPACE_MD),
        )
        addView(
            icon,
            LayoutParams(ui.px(Tokens.ICON_MD), ui.px(Tokens.ICON_MD)).apply {
                marginEnd = ui.px(Tokens.SPACE_LG)
            },
        )
        val texts =
            LinearLayout(ui.context).apply {
                orientation = VERTICAL
                addView(titleView)
                addView(supportingView)
            }
        addView(texts, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
        setShape(true, true)
    }

    fun icon(image: VectorImage?): ListItem = apply {
        icon.setImageDrawable(
            image?.let { VectorIcon(it, ui.px(Tokens.ICON_MD), ui.colors.onSurfaceVariant) }
        )
        icon.visibility = if (image == null) GONE else VISIBLE
    }

    fun title(value: CharSequence?): ListItem = apply { titleView.text = value }

    fun supporting(value: CharSequence?): ListItem = apply {
        supportingView.text = value
        supportingView.visibility = if (value.isNullOrEmpty()) GONE else VISIBLE
    }

    /** Replaces the trailing view; null removes it. */
    fun trailing(view: View?): ListItem = apply {
        trailingView?.let(::removeView)
        trailingView = view
        view?.let {
            addView(
                it,
                LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply {
                    marginStart = ui.px(Tokens.SPACE_LG)
                },
            )
        }
    }

    fun chevron(): ListItem =
        trailing(ui.icon(Icons.CHEVRON_RIGHT, Tokens.ICON_MD, ui.colors.onSurfaceVariant))

    fun onClick(listener: View.OnClickListener?): ListItem = apply {
        setOnClickListener(listener)
        isFocusable = true
    }

    override fun setEnabled(enabled: Boolean) {
        super.setEnabled(enabled)
        alpha = if (enabled) 1f else Tokens.DISABLED
    }

    internal fun setShape(first: Boolean, last: Boolean) {
        val top = (if (first) Tokens.RADIUS_LG else Tokens.RADIUS_SM).toFloat()
        val bottom = (if (last) Tokens.RADIUS_LG else Tokens.RADIUS_SM).toFloat()
        background =
            ui.ripple(
                ui.shape(ui.colors.surfaceContainer, top, bottom),
                top,
                bottom,
                ui.colors.onSurface,
            )
    }
}
