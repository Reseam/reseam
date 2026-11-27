// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import app.reseam.runtime.ui.VectorImage
import org.json.JSONException
import org.json.JSONObject

internal sealed class SchemaSetting(json: JSONObject) {
    val key: String = json.getString("key")

    val title: DisplayText? = json.optionalText("title")
    val summary: DisplayText? = json.optionalText("summary")
    val icon: VectorImage? = VectorImage.read(json.optJSONObject("icon"))

    val restart: Boolean = json.getBoolean("restart")

    val enabledBy: String? = json.optionalString("enabledBy")

    val code: Set<RowCode> = json.getJSONArray("code").strings().mapTo(mutableSetOf(), RowCode::of)

    class Toggle(json: JSONObject) : SchemaSetting(json) {
        val defaultValue: Boolean = json.getBoolean("default")
        val covers: List<String> = json.getJSONArray("covers").strings()
        val summaryOn: DisplayText? = json.optionalText("summaryOn")
        val summaryOff: DisplayText? = json.optionalText("summaryOff")

        fun summary(on: Boolean): DisplayText? = (if (on) summaryOn else summaryOff) ?: summary
    }

    sealed class Textual(json: JSONObject) : SchemaSetting(json) {
        val defaultValue: String = json.getString("default")
    }

    class Text(json: JSONObject) : Textual(json) {
        val multiline: Boolean = json.getBoolean("multiline")

        val showValue: Boolean = json.getBoolean("showValue")
    }

    class Number(json: JSONObject) : SchemaSetting(json) {
        val defaultValue: Double = json.getDouble("default")
        val slider = Slider(json.getJSONObject("slider"))
    }

    class Range(json: JSONObject) : SchemaSetting(json) {
        val defaultValue: NumberRange =
            json.getJSONObject("default").let {
                NumberRange(it.getDouble("start"), it.getDouble("end"))
            }
        val slider = Slider(json.getJSONObject("slider"))
    }

    class Color(json: JSONObject) : SchemaSetting(json) {
        val defaultValue: Int = json.getLong("default").toInt()

        val opacity: Boolean = json.getBoolean("opacity")

        val choice: Choice? = json.optionalObject("choice")?.let(::Choice)
    }

    class Folder(json: JSONObject) : Textual(json)

    class File(json: JSONObject) : Textual(json) {
        val mimeTypes: List<String> = json.getJSONArray("mimeTypes").strings()
    }

    class Choice(json: JSONObject) : Textual(json) {
        val showValue: Boolean = json.getBoolean("showValue")

        val options =
            json.getJSONArray("choices").objects().map { option ->
                Option(option.getString("value"), option.text("title"))
            }

        data class Option(val value: String, val title: DisplayText)

        fun index(stored: String): Int =
            options.indexOfFirst { it.value == stored }.takeIf { it >= 0 }
                ?: options.indexOfLast { it.value == defaultValue }.coerceAtLeast(0)
    }

    class Action(json: JSONObject) : SchemaSetting(json)

    class Note(json: JSONObject) : SchemaSetting(json)
}

internal enum class RowCode(val tag: String) {
    SUMMARY("summary"),
    ENABLED("enabled"),
    VALIDATE("validate"),
    CHANGE("change"),
    VIEW("view");

    companion object {
        fun of(tag: String): RowCode =
            entries.firstOrNull { it.tag == tag } ?: throw JSONException("Unknown row code: $tag")
    }
}

internal class Slider(json: JSONObject) {
    val min: Double = json.getDouble("min")
    val max: Double = json.getDouble("max")
    val step: Double = json.getDouble("step")
    val unit: String = json.getString("unit")
    val logarithmic: Boolean = json.getBoolean("logarithmic")

    val dialog: Boolean = json.getBoolean("dialog")
    val scale: Double = json.getDouble("scale")
    val minLabel: DisplayText? = json.optionalText("minLabel")
    val maxLabel: DisplayText? = json.optionalText("maxLabel")
}

internal fun setting(json: JSONObject): SchemaSetting =
    when (val type = json.getString("type")) {
        "toggle" -> SchemaSetting.Toggle(json)
        "text" -> SchemaSetting.Text(json)
        "number" -> SchemaSetting.Number(json)
        "range" -> SchemaSetting.Range(json)
        "color" -> SchemaSetting.Color(json)
        "folder" -> SchemaSetting.Folder(json)
        "file" -> SchemaSetting.File(json)
        "choice" -> SchemaSetting.Choice(json)
        "action" -> SchemaSetting.Action(json)
        "note" -> SchemaSetting.Note(json)
        else -> throw JSONException("Unknown setting type: $type")
    }
