// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

internal fun Json.setting(setting: Setting<*>, icon: Vector?, rows: RowWiring): Unit = obj {
    field("type", setting.kind.tag)
    field("key", setting.key)
    field("title") { text(setting.title) }
    field("summary") { text(setting.summary) }
    field("icon") { vector(icon) }
    field("restart", setting.takesEffect == TakesEffect.ON_RESTART)
    field("enabledBy", rows.enabler(setting.key))
    field("code") { array(rows.codes(setting.key)) { value(it.tag) } }
    when (setting) {
        is ToggleSetting -> {
            field("default", setting.default)
            field("summaryOn") { text(setting.summaryOn) }
            field("summaryOff") { text(setting.summaryOff) }
            field("covers") { array(setting.covers) { value(it.key) } }
        }
        is TextSetting -> {
            field("default", setting.default)
            field("multiline", setting.multiline)
            field("showValue", setting.showValue)
        }
        is NumberSetting -> {
            field("default", setting.default)
            field("slider") { slider(setting.slider) }
        }
        is RangeSetting -> {
            field("default") { range(setting.default) }
            field("slider") { slider(setting.slider) }
        }
        is ColorSetting -> {
            field("default", setting.default.toUInt().toLong())
            field("opacity", setting.opacity)
            field("choice") {
                val choice = setting.choice
                if (choice == null) nothing() else setting(choice, null, rows)
            }
        }
        is FolderSetting -> field("default", setting.default)
        is FileSetting -> {
            field("default", setting.default)
            field("mimeTypes") { array(setting.mimeTypes) { value(it) } }
        }
        is ActionSetting,
        is NoteSetting -> {}
        is ChoiceSetting -> {
            field("showValue", setting.showValue)
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

internal fun Json.range(range: ClosedFloatingPointRange<Double>) = obj {
    field("start", range.start)
    field("end", range.endInclusive)
}

private fun Json.slider(slider: Slider) = obj {
    field("min", slider.min)
    field("max", slider.max)
    field("step", slider.step)
    field("unit", slider.unit)
    field("logarithmic", slider.logarithmic)
    field("dialog", slider.dialog)
    field("scale", slider.scale)
    field("minLabel") { text(slider.minLabel) }
    field("maxLabel") { text(slider.maxLabel) }
}

internal fun Json.text(text: DisplayText?) =
    when (text) {
        null -> nothing()
        is StringRef ->
            obj {
                field("table", text.table.id)
                field("name", text.name)
            }
        is Verbatim -> obj { field("text", text.text) }
    }
