// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import app.reseam.patch.FileScope
import app.reseam.patch.types.RunningBundleInfo

/**
 * A bundle's settings as the app runtime reads them: `assets/reseam/<bundle>/settings.json`, the
 * brand's font and the bundle's string tables beside it, and the app runtime's own strings.
 */
internal class SettingsFiles(
    private val bundle: RunningBundleInfo,
    private val brand: Brand,
    private val about: About?,
    contributions: List<SettingsContribution>,
    layout: List<SettingsContribution>,
    private val placements: Map<String, PagePlacement>,
    private val extraTables: List<StringTable>,
    private val iconKeys: Set<String>?,
) {
    private val placement = SettingsPlacement(contributions, layout)
    private val sections = placement.sections
    private val pages = pagesOf(sections)
    private val directory = "assets/reseam/${bundle.name}"

    fun write(files: FileScope) {
        val schema =
            settingsSchema(
                SchemaSource(
                    bundle.name,
                    bundle.version.ifEmpty { null },
                    bundle.engine,
                    bundle.license.ifEmpty { null },
                ),
                brand,
                brand.font?.let { FONT },
                about,
                SchemaScreen(
                    pages,
                    sections,
                    placement.stored,
                    placements,
                    RowWiring.OF_RUN,
                    ::shownIcon,
                ),
            )
        files.write("$directory/$SCHEMA", schema.toByteArray(Charsets.UTF_8))
        brand.font?.let { files.writeStored("$directory/$FONT", it.file) }
        (texts().map { it.table } + extraTables).distinct().forEach {
            writeTable(files, it, "$directory/strings/${it.id}")
        }
        writeTable(files, RuntimeStrings, "assets/reseam/runtime/strings")
    }

    private fun texts(): List<StringRef> {
        val settings = sections.flatMap { it.settings } + placement.stored
        val sliders = settings.mapNotNull {
            when (it) {
                is NumberSetting -> it.slider
                is RangeSetting -> it.slider
                else -> null
            }
        }
        val links = about?.let { it.links + it.actions }.orEmpty()
        return sections.mapNotNull { it.title } +
            links.map { it.label }.filterIsInstance<StringRef>() +
            pages.flatMap { listOfNotNull(it.title, it.summary) } +
            placements.values.mapNotNull { it.heading } +
            settings.flatMap { listOfNotNull(it.title, it.summary) } +
            settings.filterIsInstance<ChoiceSetting>().flatMap { setting ->
                setting.choices.map { it.title }.filterIsInstance<StringRef>()
            } +
            sliders.flatMap { listOfNotNull(it.minLabel, it.maxLabel) }
    }

    private fun writeTable(files: FileScope, table: StringTable, target: String) {
        for (locale in StringTableFiles.locales(table)) {
            files.write("$target/$locale.json", StringTableFiles.json(table, locale))
        }
    }

    private fun shownIcon(id: String, icon: Vector?) = icon.takeIf {
        iconKeys == null || id in iconKeys
    }

    private companion object {
        const val SCHEMA = "settings.json"
        const val FONT = "font.ttf"
    }

    /** The engine build packs the app runtime's strings into the patch SDK. */
    private object RuntimeStrings : StringTable("runtime", emptySet())
}

private fun pagesOf(sections: List<SettingsSection>): List<SettingsPage> {
    val pages = linkedMapOf<String, SettingsPage>()
    fun add(page: SettingsPage) {
        page.parent?.let(::add)
        val previous = pages.putIfAbsent(page.id, page)
        require(previous == null || previous == page) { "Conflicting settings page '${page.id}'" }
    }
    sections.forEach { it.page?.let(::add) }
    return pages.values.sortedBy { it.order }
}
