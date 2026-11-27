// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import app.reseam.runtime.ui.Icons
import app.reseam.runtime.ui.PageLayout
import app.reseam.runtime.ui.Tokens
import app.reseam.runtime.ui.Ui
import app.reseam.runtime.ui.activity
import app.reseam.runtime.ui.alignToScreen
import java.text.Collator

/** A built settings page: the view to show, and the part of it that scrolls. */
internal class BuiltPage(val view: View, val scroll: ScrollView)

internal fun settingsPage(
    bundle: ReseamBundle,
    ui: Ui,
    pageId: String,
    back: () -> Unit,
    open: (String) -> Unit,
    toggleTheme: () -> Unit,
    setLanguage: (String?) -> Unit,
    restartApp: () -> Unit,
): BuiltPage {
    val schema = bundle.schema
    val root = pageId.isEmpty()
    val container =
        PageLayout(ui).apply {
            setBackgroundColor(ui.colors.surface)
            val topBar =
                ui.topBar(
                    if (root) RuntimeStrings.settings
                    else schema.page(pageId).title.resolve(bundle.name),
                    Runnable(back),
                )
            if (root) {
                topBar.action(Icons.LANGUAGE, RuntimeStrings.language) {
                    chooseLanguage(ui, setLanguage)
                }
                val dark = ui.theme.dark
                topBar.action(
                    if (dark) Icons.SUN else Icons.MOON,
                    if (dark) RuntimeStrings.useLightTheme else RuntimeStrings.useDarkTheme,
                ) {
                    toggleTheme()
                }
                topBar.action(Icons.RESTART, RuntimeStrings.restartApp) { restartApp() }
            }
            addView(topBar, matchWidth())
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
    val groups = pageContents(schema, pageId)
    for (group in groups) {
        val title = group.title
        val items = ui.itemGroup()
        group.rows.forEach { row ->
            items.add(
                when (row) {
                    is PageRow.Setting -> rows.create(row.setting)
                    is PageRow.Link ->
                        ui.listItem(row.page.title.resolve(bundle.name))
                            .icon(row.page.icon)
                            .supporting(row.page.summary?.resolve(bundle.name))
                            .chevron()
                            .onClick { open(row.page.id) }
                }
            )
        }
        if (title != null) {
            content.addView(ui.sectionLabel(title.resolve(bundle.name)))
            content.addView(items, matchWidth())
        } else content.addView(items, matchWidth().apply { topMargin = ui.px(Tokens.SPACE_LG) })
    }
    if (groups.isEmpty()) content.addView(settingsMessage(ui, RuntimeStrings.noSettings))
    if (root && schema.storesValues) {
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
                )
                .add(
                    ui.listItem(RuntimeStrings.settingsAsText)
                        .icon(Icons.CLIPBOARD)
                        .supporting(RuntimeStrings.settingsAsTextSummary)
                        .onClick { settingsAsText(activity(ui.context), bundle) }
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
    return BuiltPage(container, scroll)
}

/** Lists each language by its own name, after the choice to follow the app's language. */
private fun chooseLanguage(ui: Ui, setLanguage: (String?) -> Unit) {
    val languages =
        RuntimeStrings.table.languages
            .map { (tag, locale) ->
                tag to locale.getDisplayName(locale).replaceFirstChar { it.titlecase(locale) }
            }
            .sortedWith(compareBy(Collator.getInstance()) { it.second })
    val tags = listOf(null) + languages.map { it.first }
    val names = listOf(RuntimeStrings.languageSameAsApp) + languages.map { it.second }
    ui.dialog()
        .title(RuntimeStrings.language)
        .choices(names.toTypedArray(), tags.indexOf(Appearance.language)) { setLanguage(tags[it]) }
        .dismissAction(RuntimeStrings.cancel)
        .show()
}

internal fun settingsMessage(ui: Ui, text: String): TextView =
    ui.text(text, ui.type.bodyMedium, ui.colors.onSurfaceVariant).alignToScreen().apply {
        setPadding(0, ui.px(Tokens.SPACE_LG), 0, ui.px(Tokens.SPACE_LG))
    }

private fun vertical(ui: Ui): LinearLayout =
    LinearLayout(ui.context).apply { orientation = LinearLayout.VERTICAL }

private fun matchWidth(): LinearLayout.LayoutParams =
    LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.WRAP_CONTENT,
    )
