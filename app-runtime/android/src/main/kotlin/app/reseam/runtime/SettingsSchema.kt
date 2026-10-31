// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import app.reseam.runtime.ui.VectorImage
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject

internal class SettingsSchema(json: JSONObject) {
    val version: String? = json.getJSONObject("bundle").optionalString("version")
    val engine: String = json.getString("engine")
    val license: String? = json.optionalString("license")
    val about: About? = json.optJSONObject("about")?.let(::About)
    val pages = json.getJSONArray("pages").objects().map(::Page)
    val sections = json.getJSONArray("sections").objects().map(::Section)
    val settings = sections.flatMap { it.settings }.distinctBy { it.key }

    fun page(id: String): Page =
        pages.firstOrNull { it.id == id }
            ?: throw IllegalArgumentException("Unknown settings page: $id")

    fun path(id: String, bundle: String): String {
        if (id.isEmpty()) return ""
        val page = page(id)
        val parent = path(page.parent, bundle)
        val title = page.title.resolve(bundle)
        return if (parent.isEmpty()) title else "$parent › $title"
    }

    fun setting(key: String): Setting =
        settings.firstOrNull { it.key == key }
            ?: throw IllegalArgumentException("Unknown setting: $key")

    class Page(json: JSONObject) {
        val id: String = json.getString("id")
        val title: DisplayText = json.text("title")
        val summary: DisplayText? = json.optionalText("summary")
        val icon: VectorImage? = VectorImage.read(json.optJSONObject("icon"))
        val parent: String = json.optionalString("parent") ?: ""
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
        val title: DisplayText = json.text("title")
        val settings: List<Setting> =
            json.getJSONArray("settings").objects().map { setting ->
                when (val type = setting.getString("type")) {
                    "toggle" -> Toggle(setting)
                    "text" -> Text(setting)
                    "folder" -> Folder(setting)
                    "choice" -> Choice(setting)
                    else -> throw JSONException("Unknown setting type: $type")
                }
            }
    }

    sealed class Setting(json: JSONObject) {
        val key: String = json.getString("key")
        val title: DisplayText = json.text("title")
        val summary: DisplayText? = json.optionalText("summary")
        val icon: VectorImage? = VectorImage.read(json.optJSONObject("icon"))
    }

    class Toggle(json: JSONObject) : Setting(json) {
        val defaultValue: Boolean = json.getBoolean("default")
        val covers: List<String> = json.getJSONArray("covers").strings()
    }

    class Text(json: JSONObject) : Setting(json) {
        val defaultValue: String = json.getString("default")
        val multiline: Boolean = json.getBoolean("multiline")
    }

    class Folder(json: JSONObject) : Setting(json) {
        val defaultValue: String = json.getString("default")
    }

    class Choice(json: JSONObject) : Setting(json) {
        val defaultValue: String = json.getString("default")
        val options =
            json.getJSONArray("choices").objects().map { option ->
                Option(option.getString("value"), option.text("title"))
            }

        data class Option(val value: String, val title: DisplayText)

        /**
         * An unlisted stored value uses the default, or the first choice if the default is absent.
         */
        fun index(stored: String): Int =
            options.indexOfFirst { it.value == stored }.takeIf { it >= 0 }
                ?: options.indexOfLast { it.value == defaultValue }.coerceAtLeast(0)
    }
}

private fun JSONObject.optionalString(key: String): String? =
    if (isNull(key)) null else getString(key)

private fun JSONObject.text(key: String): DisplayText {
    val text = getJSONObject(key)
    return if (text.has("text")) Verbatim(text.getString("text"))
    else TextRef(text.getString("table"), text.getString("name"))
}

private fun JSONObject.optionalText(key: String): DisplayText? =
    if (isNull(key)) null else text(key)

private fun JSONArray.objects(): List<JSONObject> = List(length()) { getJSONObject(it) }

private fun JSONArray.strings(): List<String> = List(length()) { getString(it) }
