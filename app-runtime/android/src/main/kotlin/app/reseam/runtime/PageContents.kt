// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

internal sealed interface PageRow {
    class Setting(val setting: SchemaSetting) : PageRow

    class Link(val page: SettingsSchema.Page) : PageRow
}

internal class RowGroup(val title: DisplayText?) {
    val rows = mutableListOf<PageRow>()
}

internal fun pageContents(schema: SettingsSchema, pageId: String): List<RowGroup> {
    val sections = schema.sections.filter { it.page == pageId && it.settings.isNotEmpty() }
    val shownKeys = sections.flatMap { it.settings }.mapTo(hashSetOf()) { it.key }
    val (placed, leading) =
        schema.pages.filter { it.parent == pageId }.partition { it.after in shownKeys }
    val groups = mutableListOf<RowGroup>()
    if (leading.isNotEmpty()) groups += RowGroup(null).apply { rows += leading.map(PageRow::Link) }
    var current: RowGroup? = null
    for (section in sections) {
        var group =
            current.takeIf { section.title == null }
                ?: RowGroup(section.title).also { groups += it }
        for (setting in section.settings) {
            group.rows += PageRow.Setting(setting)
            for (page in placed.filter { it.after == setting.key }) {
                page.heading?.let { heading -> group = RowGroup(heading).also { groups += it } }
                group.rows += PageRow.Link(page)
            }
        }
        current = group
    }
    return groups
}
