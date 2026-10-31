// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.view.Gravity
import android.view.ViewGroup
import android.widget.LinearLayout

internal fun dialogHeader(
    ui: Ui,
    icon: VectorImage?,
    title: CharSequence?,
    message: CharSequence?,
    padding: Int,
): LinearLayout =
    LinearLayout(ui.context).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(padding, 0, padding, 0)
        icon?.let {
            addView(
                ui.icon(it, Tokens.ICON_MD, ui.colors.onSurfaceVariant),
                LinearLayout.LayoutParams(ui.px(Tokens.ICON_MD), ui.px(Tokens.ICON_MD)).apply {
                    gravity = Gravity.CENTER_HORIZONTAL
                    bottomMargin = ui.px(Tokens.SPACE_LG)
                },
            )
        }
        title?.let {
            val label =
                ui.text(it, ui.type.headlineSmall, ui.colors.onSurface).apply {
                    gravity = if (icon != null) Gravity.CENTER_HORIZONTAL else Gravity.START
                }
            addView(
                label,
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                ),
            )
        }
        message?.let {
            addView(
                ui.text(it, ui.type.bodyMedium, ui.colors.onSurfaceVariant).apply {
                    setPadding(0, if (title != null) ui.px(Tokens.SPACE_LG) else 0, 0, 0)
                }
            )
        }
    }
