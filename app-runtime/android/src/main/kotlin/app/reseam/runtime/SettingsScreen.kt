// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.graphics.Rect
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import app.reseam.runtime.ui.Icons
import app.reseam.runtime.ui.Tokens
import app.reseam.runtime.ui.Ui

internal fun settingsPage(
    bundle: ReseamBundle,
    ui: Ui,
    pageId: String,
    back: () -> Unit,
    open: (String) -> Unit,
): View {
    val schema = bundle.schema
    val root = pageId.isEmpty()
    val container =
        SettingsPageLayout(ui).apply {
            setBackgroundColor(ui.colors.surface)
            // Hosts targeting API 35 draw edge to edge; keep the bar below the status bar.
            fitsSystemWindows = true
            addView(
                ui.topBar(
                    if (root) RuntimeStrings.settings
                    else schema.page(pageId).title.resolve(bundle.name),
                    Runnable(back),
                ),
                matchWidth(),
            )
        }
    val body =
        vertical(ui).apply {
            setPadding(
                ui.px(Tokens.SPACE_LG),
                0,
                ui.px(Tokens.SPACE_LG),
                ui.px(Tokens.SPACE_XXL),
            )
        }
    val scroll = ScrollView(ui.context).apply { addView(body) }
    container.addView(
        scroll,
        LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f),
    )
    val rows = SettingRows(bundle, ui)
    rows.follow(container)
    val content = vertical(ui)
    if (root) content.addView(aboutCard(ui, bundle), matchWidth())
    val pages = ui.itemGroup()
    schema.pages
        .filter { it.parent == pageId }
        .forEach { page ->
            pages.add(
                ui.listItem(page.title.resolve(bundle.name))
                    .icon(page.icon)
                    .supporting(page.summary?.resolve(bundle.name))
                    .chevron()
                    .onClick { open(page.id) }
            )
        }
    if (pages.childCount > 0)
        content.addView(pages, matchWidth().apply { topMargin = ui.px(Tokens.SPACE_LG) })
    val sections = schema.sections.filter { it.page == pageId && it.settings.isNotEmpty() }
    sections.forEach { section ->
        content.addView(ui.sectionLabel(section.title.resolve(bundle.name)))
        val group = ui.itemGroup()
        section.settings.forEach { group.add(rows.create(it)) }
        content.addView(group, matchWidth())
    }
    if (pages.childCount == 0 && sections.isEmpty())
        content.addView(settingsMessage(ui, RuntimeStrings.noSettings))
    if (root && schema.settings.isNotEmpty()) {
        content.addView(ui.sectionLabel(RuntimeStrings.backup))
        content.addView(
            ui.itemGroup()
                .add(
                    ui.listItem(RuntimeStrings.exportSettings)
                        .icon(Icons.UPLOAD)
                        .supporting(RuntimeStrings.exportSettingsSummary)
                        .onClick { exportSettings(activity(ui.context), bundle) }
                )
                .add(
                    ui.listItem(RuntimeStrings.importSettings)
                        .icon(Icons.DOWNLOAD)
                        .supporting(RuntimeStrings.importSettingsSummary)
                        .onClick { importSettings(activity(ui.context), bundle) }
                ),
            matchWidth(),
        )
    }
    if (root) {
        val results = SearchResults(ui, bundle, rows, content)
        val search = ui.searchBar(RuntimeStrings.searchSettings)
        search.onQueryChange {
            results.show(it)
            scroll.scrollTo(0, 0)
        }
        body.addView(
            search,
            matchWidth().apply {
                topMargin = ui.px(Tokens.SPACE_XS)
                bottomMargin = ui.px(Tokens.SPACE_LG)
            },
        )
        body.addView(results.view, matchWidth())
    }
    body.addView(content, matchWidth())
    return container
}

internal fun settingsMessage(ui: Ui, text: String): TextView =
    ui.text(text, ui.type.bodyMedium, ui.colors.onSurfaceVariant).apply {
        setPadding(0, ui.px(Tokens.SPACE_LG), 0, ui.px(Tokens.SPACE_LG))
    }

private class SettingsPageLayout(ui: Ui) : LinearLayout(ui.context) {
    init {
        orientation = VERTICAL
        // Give initial focus somewhere to go other than the search editor.
        isFocusableInTouchMode = true
    }

    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        val editor = findFocus() as? EditText
        if (event.action == MotionEvent.ACTION_DOWN && editor != null) {
            val bounds = Rect()
            (editor.parent as View).getGlobalVisibleRect(bounds)
            if (!bounds.contains(event.rawX.toInt(), event.rawY.toInt())) {
                editor.clearFocus()
                context
                    .getSystemService(InputMethodManager::class.java)
                    .hideSoftInputFromWindow(editor.windowToken, 0)
            }
        }
        return super.dispatchTouchEvent(event)
    }
}

private fun vertical(ui: Ui): LinearLayout =
    LinearLayout(ui.context).apply { orientation = LinearLayout.VERTICAL }

private fun matchWidth(): LinearLayout.LayoutParams =
    LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.WRAP_CONTENT,
    )
