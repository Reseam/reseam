// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.os.Build
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import app.reseam.runtime.ui.ListItem
import app.reseam.runtime.ui.Tokens
import app.reseam.runtime.ui.Ui

internal fun SettingRows.number(item: ListItem, number: SchemaSetting.Number) =
    sliderRow(
        item,
        number,
        number.slider,
        SliderValues(
            read = { listOf(settings.getNumber(number.key, number.defaultValue)) },
            write = { settings.setNumber(number.key, it.single()) },
            text = { NumberText.format(it.single()) },
            parse = { NumberText.parse(it)?.let(::listOf) },
        ),
    )

internal fun SettingRows.range(item: ListItem, range: SchemaSetting.Range) =
    sliderRow(
        item,
        range,
        range.slider,
        SliderValues(
            read = {
                settings.getRange(range.key, range.defaultValue).let { listOf(it.start, it.end) }
            },
            write = { (start, end) -> settings.setRange(range.key, NumberRange(start, end)) },
            text = { (start, end) -> NumberText.format(NumberRange(start, end)) },
            parse = { text -> NumberText.parseRange(text)?.let { listOf(it.start, it.end) } },
        ),
    )

internal class SliderValues(
    val read: () -> List<Double>,
    val write: (List<Double>) -> Unit,
    val text: (List<Double>) -> String,
    val parse: (String) -> List<Double>?,
)

private fun SettingRows.sliderRow(
    item: ListItem,
    setting: SchemaSetting,
    slider: Slider,
    values: SliderValues,
) {
    val scale = SliderScale(slider)
    item.supporting(summary(setting))
    if (slider.dialog) {
        item.onClick {
            var picked = values.read()
            val picker =
                SliderPicker(ui, bundle.name, scale, picked.size) { moved, _ -> picked = moved }
            picker.set(picked)
            ui.dialog()
                .title(setting.title?.resolve(bundle.name))
                .message(summary(setting))
                .view(picker.view)
                .dismissAction(RuntimeStrings.cancel)
                .action(RuntimeStrings.save) { store(setting) { values.write(picked) } }
                .show()
        }
        return
    }
    val picker =
        SliderPicker(ui, bundle.name, scale, values.read().size) { moved, settled ->
            if (settled) store(setting) { values.write(moved) }
        }
    onChange(setting.key) { picker.set(values.read()) }
    item.below(picker.view)
    item.onClick {
        editText(
            ui,
            EditedText(
                setting.title?.resolve(bundle.name),
                summary(setting),
                values.text(values.read()),
                1,
            ),
            values.text(values.read()),
            check = { entered ->
                RuntimeStrings.valueOutOfRange(
                        NumberText.format(slider.min),
                        NumberText.format(slider.max),
                    )
                    .takeUnless { values.parse(entered)?.all(scale::covers) == true }
            },
        ) { entered ->
            store(setting) { values.write(checkNotNull(values.parse(entered))) }
        }
    }
}

private class SliderPicker(
    ui: Ui,
    private val bundle: String,
    private val scale: SliderScale,
    thumbs: Int,
    onChange: (values: List<Double>, settled: Boolean) -> Unit,
) {
    private val slider = ui.slider(scale.steps, thumbs)
    private val shown =
        ui.text("", ui.type.labelLarge, ui.colors.onSurface).apply {
            gravity = Gravity.CENTER_HORIZONTAL
        }
    val view =
        LinearLayout(ui.context).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, ui.px(Tokens.SPACE_SM), 0, 0)
            addView(
                LinearLayout(ui.context).apply {
                    addView(end(ui, scale.valueAt(0)))
                    addView(View(ui.context), LinearLayout.LayoutParams(0, 0, 1f))
                    addView(end(ui, scale.valueAt(scale.steps)))
                }
            )
            addView(slider)
            addView(shown)
        }

    init {
        slider.setOnChangeListener { view, _, settled ->
            val values = view.positions.map(scale::valueAt)
            show(values)
            onChange(values, settled)
        }
    }

    fun set(values: List<Double>) {
        slider.setPosition(*values.map(scale::positionOf).toIntArray())
        show(values)
    }

    private fun end(ui: Ui, value: Double) =
        ui.text(scale.label(value, bundle), ui.type.labelMedium, ui.colors.onSurfaceVariant)

    private fun show(values: List<Double>) {
        shown.text = values.joinToString(" – ") { scale.label(it, bundle) }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) slider.stateDescription = shown.text
    }
}
