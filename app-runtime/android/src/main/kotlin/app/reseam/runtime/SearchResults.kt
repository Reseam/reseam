// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import app.reseam.runtime.ui.ListItem
import app.reseam.runtime.ui.Ui
import java.util.Locale

internal class SearchResults(
    private val ui: Ui,
    private val bundle: ReseamBundle,
    private val rows: SettingRows,
    private val page: View,
) {
    val view: LinearLayout =
        LinearLayout(ui.context).apply {
            orientation = LinearLayout.VERTICAL
            visibility = View.GONE
        }
    private val empty = settingsMessage(ui, RuntimeStrings.noSearchResults)
    private val labels = mutableMapOf<SettingsSchema.Section, View>()
    private val items = mutableMapOf<String, ListItem>()

    fun show(query: String) {
        val normalized = query.trim().lowercase(Locale.ROOT)
        val words = if (normalized.isEmpty()) emptyList() else normalized.split(WHITESPACE)
        val searching = words.isNotEmpty()
        page.visibility = if (searching) View.GONE else View.VISIBLE
        view.visibility = if (searching) View.VISIBLE else View.GONE
        view.removeAllViews()
        if (!searching) return
        for (section in bundle.schema.sections) {
            val matches = section.settings.filter { matches(it, words) }
            if (matches.isEmpty()) continue
            val group = ui.itemGroup()
            view.addView(labels.getOrPut(section) { label(section) })
            view.addView(group)
            matches.forEach { setting ->
                val item = items.getOrPut(setting.key) { rows.create(setting) }
                // Reused items still belong to the previous query's group.
                (item.parent as? ViewGroup)?.removeView(item)
                group.add(item)
            }
        }
        if (view.childCount == 0) view.addView(empty)
    }

    private fun label(section: SettingsSchema.Section): View {
        val path = bundle.schema.path(section.page, bundle.name)
        val title = section.title.resolve(bundle.name)
        return ui.sectionLabel(if (path.isEmpty()) title else "$path › $title")
    }

    private fun matches(setting: SettingsSchema.Setting, words: List<String>): Boolean {
        val title = setting.title.resolve(bundle.name)
        val summary = setting.summary?.resolve(bundle.name)
        val text = (if (summary == null) title else "$title\n$summary").lowercase(Locale.ROOT)
        return words.all { it in text }
    }
}

private val WHITESPACE = Regex("\\s+")
