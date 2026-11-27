// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import app.reseam.runtime.ui.VectorImage
import org.json.JSONObject

internal class SettingsSchema(json: JSONObject) {
    val version: String? = json.getJSONObject("bundle").optionalString("version")
    val engine: String = json.getString("engine")
    val license: String? = json.optionalString("license")
    val about: About? = json.optionalObject("about")?.let(::About)
    val pages = json.getJSONArray("pages").objects().map(::Page)
    val sections = json.getJSONArray("sections").objects().map(::Section)

    val settings =
        (sections.flatMap { it.settings } + json.getJSONArray("stored").objects().map(::setting))
            .distinctBy { it.key }
    private val byKey = settings.associateBy { it.key }

    fun page(id: String): Page =
        pages.firstOrNull { it.id == id }
            ?: throw IllegalArgumentException("Unknown settings page: $id")

    fun path(id: String, bundle: String): String {
        if (id.isEmpty()) return ""
        val page = page(id)
        val parent = path(page.parent, bundle)
        val title = page.title.resolve(bundle)
        return if (parent.isEmpty()) title.toString() else "$parent › $title"
    }

    fun setting(key: String): SchemaSetting =
        byKey[key] ?: throw IllegalArgumentException("Unknown setting: $key")

    fun settingOrNull(key: String): SchemaSetting? = byKey[key]

    inline fun <reified S : SchemaSetting> settingOf(key: String): S? = settingOrNull(key) as? S

    val storesValues: Boolean
        get() = settings.any { it !is SchemaSetting.Action && it !is SchemaSetting.Note }

    class Page(json: JSONObject) {
        val id: String = json.getString("id")
        val title: DisplayText = json.text("title")
        val summary: DisplayText? = json.optionalText("summary")
        val icon: VectorImage? = VectorImage.read(json.optJSONObject("icon"))
        val parent: String = json.optionalString("parent") ?: ""

        val after: String? = json.optionalString("after")

        val heading: DisplayText? = json.optionalText("heading")
    }

    class About(json: JSONObject) {
        val links = json.getJSONArray("links").objects().map(::Link)
        val actions = json.getJSONArray("actions").objects().map(::Link)
    }

    class Link(json: JSONObject) {
        val label: DisplayText = json.text("label")
        val url: String = json.getString("url")
        val icon: VectorImage = checkNotNull(VectorImage.read(json.getJSONObject("icon")))
    }

    class Section(json: JSONObject) {
        val page: String = json.optionalString("page") ?: ""
        val title: DisplayText? = json.optionalText("title")
        val settings: List<SchemaSetting> = json.getJSONArray("settings").objects().map(::setting)
    }
}
