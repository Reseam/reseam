// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
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
    contributions: List<SettingsSection>,
    private val extraTables: List<StringTable>,
) {
    private val sections = contributions.filter { it.settings.isNotEmpty() }
    private val directory = "assets/reseam/${bundle.name}"

    fun write(files: FileScope) {
        files.write("$directory/$SCHEMA", schema().toByteArray(Charsets.UTF_8))
        brand.font?.let { files.writeStored("$directory/$FONT", it.file) }
        (texts().map { it.table } + extraTables).distinct().forEach {
            writeTable(files, it, "$directory/strings/${it.id}")
        }
        writeTable(files, RuntimeStrings, "assets/reseam/runtime/strings")
    }

    private fun texts(): List<StringRef> {
        val pages =
            sections.mapNotNull { it.page }.flatMap { generateSequence(it, SettingsPage::parent) }
        val settings = sections.flatMap { it.settings }
        val links = about?.let { it.links + it.actions }.orEmpty()
        return sections.map { it.title } +
            links.map { it.label }.filterIsInstance<StringRef>() +
            pages.flatMap { listOfNotNull(it.title, it.summary) } +
            settings.flatMap { listOfNotNull(it.title, it.summary) } +
            settings.filterIsInstance<ChoiceSetting>().flatMap { setting ->
                setting.choices.map { it.title }.filterIsInstance<StringRef>()
            }
    }

    private fun writeTable(files: FileScope, table: StringTable, target: String) {
        for (locale in table.locales()) {
            files.write(
                "$target/$locale.json",
                table.resource("$locale.json").use { it.readBytes() },
            )
        }
    }

    private fun schema(): String {
        val pages = linkedMapOf<String, SettingsPage>()
        fun addPage(page: SettingsPage) {
            page.parent?.let(::addPage)
            val previous = pages.putIfAbsent(page.id, page)
            require(previous == null || previous == page) {
                "Conflicting settings page '${page.id}'"
            }
        }
        sections.forEach { it.page?.let(::addPage) }
        val merged =
            sections
                .groupBy { it.page?.id to it.title }
                .map { (location, contributions) ->
                    Triple(
                        location.first,
                        location.second,
                        contributions.flatMap { it.settings }.distinctBy { it.key },
                    )
                }
        return Json.build {
            obj {
                field("bundle") {
                    obj {
                        field("name", bundle.name)
                        field("version", bundle.version.ifEmpty { null })
                    }
                }
                field("engine", bundle.engine)
                field("license", bundle.license.ifEmpty { null })
                field("brand") { brand(brand, brand.font?.let { FONT }) }
                field("about") {
                    val about = about
                    if (about == null) nothing()
                    else
                        obj {
                            field("links") { array(about.links) { link(it) } }
                            field("actions") { array(about.actions) { link(it) } }
                        }
                }
                field("pages") {
                    array(pages.values.sortedBy { it.order }) { page ->
                        obj {
                            field("id", page.id)
                            field("title") { text(page.title) }
                            field("summary") { text(page.summary) }
                            field("icon") { vector(page.icon) }
                            field("parent", page.parent?.id)
                        }
                    }
                }
                field("sections") {
                    array(merged) { (page, title, settings) ->
                        obj {
                            field("page", page)
                            field("title") { text(title) }
                            field("settings") { array(settings) { setting(it) } }
                        }
                    }
                }
            }
        }
    }

    private fun Json.setting(setting: Setting<*>) = obj {
        field("type", setting.kind.tag)
        field("key", setting.key)
        field("title") { text(setting.title) }
        field("summary") { text(setting.summary) }
        field("icon") { vector(setting.icon) }
        when (setting) {
            is ToggleSetting -> {
                field("default", setting.default)
                field("covers") { array(setting.covers) { value(it.key) } }
            }
            is TextSetting -> {
                field("default", setting.default)
                field("multiline", setting.multiline)
            }
            is FolderSetting -> field("default", setting.default)
            is ChoiceSetting -> {
                field("default", setting.default)
                field("choices") {
                    array(setting.choices) { choice ->
                        obj {
                            field("value", choice.value)
                            field("title") { text(choice.title) }
                        }
                    }
                }
            }
        }
    }

    private fun Json.link(link: Link) = obj {
        field("label") { text(link.label) }
        field("url", link.url)
        field("icon") { vector(link.icon) }
    }

    private fun Json.text(text: DisplayText?) =
        when (text) {
            null -> nothing()
            is StringRef ->
                obj {
                    field("table", text.table.id)
                    field("name", text.name)
                }
            is Verbatim -> obj { field("text", text.text) }
        }

    private companion object {
        const val SCHEMA = "settings.json"
        const val FONT = "font.ttf"
    }

    /** The engine build packs the app runtime's strings into the patch SDK. */
    private object RuntimeStrings : StringTable("runtime", emptySet())
}
