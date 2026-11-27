// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

internal class SchemaSource(
    val bundle: String,
    val version: String?,
    val engine: String,
    val license: String?,
)

internal class SchemaScreen(
    val pages: List<SettingsPage>,
    val sections: List<SettingsSection>,
    val stored: List<Setting<*>>,
    val placements: Map<String, PagePlacement>,
    val rows: RowWiring,
    val icon: (id: String, icon: Vector?) -> Vector?,
)

internal fun settingsSchema(
    source: SchemaSource,
    brand: Brand,
    font: String?,
    about: About?,
    screen: SchemaScreen,
): String = Json.build {
    obj {
        field("bundle") {
            obj {
                field("name", source.bundle)
                field("version", source.version)
            }
        }
        field("engine", source.engine)
        field("license", source.license)
        field("brand") { brand(brand, font) }
        field("about") {
            if (about == null) nothing()
            else
                obj {
                    field("links") { array(about.links) { link(it) } }
                    field("actions") { array(about.actions) { link(it) } }
                }
        }
        field("pages") { array(screen.pages) { page(it, screen) } }
        field("stored") { array(screen.stored) { setting(it, null, screen.rows) } }
        field("sections") {
            array(screen.sections) { section ->
                obj {
                    field("page", section.page?.id)
                    field("title") { text(section.title) }
                    field("settings") {
                        array(section.settings) {
                            setting(it, screen.icon(it.key, it.icon), screen.rows)
                        }
                    }
                }
            }
        }
    }
}

private fun Json.page(page: SettingsPage, screen: SchemaScreen) = obj {
    field("id", page.id)
    field("title") { text(page.title) }
    field("summary") { text(page.summary) }
    field("icon") { vector(screen.icon(page.id, page.icon)) }
    field("parent", page.parent?.id)
    val placement = screen.placements[page.id]
    field("after", placement?.after?.key)
    field("heading") { text(placement?.heading) }
}

private fun Json.link(link: Link) = obj {
    field("label") { text(link.label) }
    field("url", link.url)
    field("icon") { vector(link.icon) }
}
