// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.content.Intent
import android.net.Uri
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import app.reseam.runtime.ui.Tokens
import app.reseam.runtime.ui.Ui

internal fun aboutCard(ui: Ui, bundle: ReseamBundle): View =
    LinearLayout(ui.context).apply {
        orientation = LinearLayout.VERTICAL
        background = ui.shape(ui.colors.surfaceContainer, Tokens.RADIUS_XL)
        val padding = ui.px(Tokens.SPACE_LG)
        setPadding(padding, padding, padding, padding)
        addView(identity(ui, bundle.schema))
        val about = bundle.schema.about ?: return@apply
        if (about.links.isNotEmpty()) addView(links(ui, bundle.name, about.links), iconRow(ui))
        if (about.actions.isNotEmpty()) addView(actions(ui, bundle.name, about.actions), spaced(ui))
    }

private fun identity(ui: Ui, schema: SettingsSchema): View =
    LinearLayout(ui.context).apply {
        gravity = Gravity.CENTER_VERTICAL
        ui.theme.logo?.let {
            addView(
                ui.icon(it, Tokens.LOGO, ui.colors.onSurface),
                LinearLayout.LayoutParams(ui.px(Tokens.LOGO), ui.px(Tokens.LOGO)).apply {
                    marginEnd = ui.px(Tokens.SPACE_LG)
                },
            )
        }
        val engine =
            schema.license?.let { RuntimeStrings.engineLicense(schema.engine, it) }
                ?: RuntimeStrings.engine(schema.engine)
        addView(
            LinearLayout(ui.context).apply {
                orientation = LinearLayout.VERTICAL
                addView(ui.text(ui.theme.brand.name, ui.type.titleLarge, ui.colors.onSurface))
                listOfNotNull(schema.version?.let(RuntimeStrings::version), engine).forEach {
                    addView(ui.text(it, ui.type.bodyMedium, ui.colors.onSurfaceVariant))
                }
            }
        )
    }

private fun links(ui: Ui, bundle: String, links: List<SettingsSchema.Link>): View =
    LinearLayout(ui.context).apply {
        links.forEach { link ->
            addView(
                ui.iconButton(link.icon, ui.colors.onSurfaceVariant, link.label.resolve(bundle)) {
                    open(it, link.url)
                }
            )
        }
    }

private fun actions(ui: Ui, bundle: String, actions: List<SettingsSchema.Link>): View =
    LinearLayout(ui.context).apply {
        actions.forEachIndexed { index, action ->
            addView(
                ui.tonalButton(action.label.resolve(bundle), action.icon) { open(it, action.url) },
                LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                    if (index > 0) marginStart = ui.px(Tokens.SPACE_SM)
                },
            )
        }
    }

private fun spaced(ui: Ui) =
    LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT,
        )
        .apply { topMargin = ui.px(Tokens.SPACE_SM) }

// Lines the first icon, not its touch target, up with the content.
private fun iconRow(ui: Ui) =
    spaced(ui).apply { marginStart = -ui.px((Tokens.TOUCH_TARGET - Tokens.ICON_MD) / 2) }

private fun open(view: View, url: String) =
    view.context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
