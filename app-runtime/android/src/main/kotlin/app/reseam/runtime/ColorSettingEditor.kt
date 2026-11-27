// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.graphics.Color
import android.view.Gravity
import android.widget.LinearLayout
import app.reseam.runtime.ui.ChoiceRow
import app.reseam.runtime.ui.Icons
import app.reseam.runtime.ui.ListItem
import app.reseam.runtime.ui.SliderView
import app.reseam.runtime.ui.Tokens
import app.reseam.runtime.ui.Ui
import app.reseam.runtime.ui.afterTextChanged

internal fun SettingRows.color(item: ListItem, color: SchemaSetting.Color) {
    val swatch = ui.colorSwatch(Tokens.ICON_MD)
    item.trailing(swatch)
    onChange(color.key) { swatch.color = settings.getColor(color.key, color.defaultValue) }
    val linked = color.choice
    if (linked == null) item.supporting(summary(color))
    else
        onChange(linked.key) {
            item.supporting(summary(color) ?: chosen(linked).title.resolve(bundle.name))
        }
    item.onClick {
        editColor(ui, bundle, color) { picked, choice ->
            store(color) { settings.setColor(color.key, picked) }
            if (linked != null && choice != null) {
                store(linked) { settings.setString(linked.key, choice) }
            }
        }
    }
}

private fun editColor(
    ui: Ui,
    bundle: ReseamBundle,
    color: SchemaSetting.Color,
    store: (color: Int, choice: String?) -> Unit,
) {
    val settings = bundle.settings
    var choice = color.choice?.let { settings.getChoice(it.key, it.defaultValue) }
    val hsv = FloatArray(3)
    var alpha = OPAQUE
    fun load(argb: Int) {
        Color.colorToHSV(argb, hsv)
        alpha = if (color.opacity) Color.alpha(argb) else OPAQUE
    }
    load(settings.getColor(color.key, color.defaultValue))
    fun current() = Color.HSVToColor(alpha, hsv)
    fun code() = ColorCode.format(current(), color.opacity)

    val preview = ui.colorSwatch(PREVIEW)
    val field = ui.textField(RuntimeStrings.colorCode)
    val hue = ui.slider(HUES)
    val saturation = ui.slider(PERCENT)
    val brightness = ui.slider(PERCENT)
    val opacity = ui.slider(OPAQUE)
    var syncing = false
    fun show(fromField: Boolean) {
        syncing = true
        preview.color = current()
        hue.setPosition(Math.round(hsv[0]))
        saturation.setPosition(Math.round(hsv[1] * PERCENT))
        brightness.setPosition(Math.round(hsv[2] * PERCENT))
        opacity.setPosition(alpha)
        if (!fromField) field.text(code())
        field.showAction(code() != ColorCode.format(color.defaultValue, color.opacity))
        syncing = false
    }
    hue.setOnChangeListener { _, position, _ ->
        hsv[0] = position.toFloat()
        show(false)
    }
    saturation.setOnChangeListener { _, position, _ ->
        hsv[1] = position / PERCENT.toFloat()
        show(false)
    }
    brightness.setOnChangeListener { _, position, _ ->
        hsv[2] = position / PERCENT.toFloat()
        show(false)
    }
    opacity.setOnChangeListener { _, position, _ ->
        alpha = position
        show(false)
    }
    // A code still being typed leaves the color as it was.
    field.field.afterTextChanged { typed ->
        if (!syncing)
            ColorCode.parse(typed)?.let {
                load(it)
                show(true)
            }
    }
    field.action(Icons.RESET, RuntimeStrings.restoreDefault) {
        load(color.defaultValue)
        show(false)
    }
    val body =
        LinearLayout(ui.context).apply {
            orientation = LinearLayout.VERTICAL
            color.choice?.let { linked ->
                addView(choices(ui, bundle, linked, choice) { picked -> choice = picked })
            }
            addView(
                preview,
                LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.WRAP_CONTENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT,
                    )
                    .apply {
                        gravity = Gravity.CENTER_HORIZONTAL
                        bottomMargin = ui.px(Tokens.SPACE_LG)
                    },
            )
            fun labelled(label: String, slider: SliderView) {
                addView(ui.text(label, ui.type.labelMedium, ui.colors.onSurfaceVariant))
                slider.contentDescription = label
                addView(slider)
            }
            labelled(RuntimeStrings.colorHue, hue)
            labelled(RuntimeStrings.colorSaturation, saturation)
            labelled(RuntimeStrings.colorBrightness, brightness)
            if (color.opacity) labelled(RuntimeStrings.colorOpacity, opacity)
            addView(
                field,
                LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT,
                    )
                    .apply { topMargin = ui.px(Tokens.SPACE_LG) },
            )
        }
    show(false)
    ui.dialog()
        .title(color.title?.resolve(bundle.name))
        .message(color.summary?.resolve(bundle.name))
        .view(body)
        .dismissAction(RuntimeStrings.cancel)
        .action(RuntimeStrings.save) { store(current(), choice) }
        .show()
}

private fun choices(
    ui: Ui,
    bundle: ReseamBundle,
    choice: SchemaSetting.Choice,
    chosen: String?,
    onChoose: (String) -> Unit,
): LinearLayout =
    LinearLayout(ui.context).apply {
        orientation = LinearLayout.VERTICAL
        fun build(selected: String?) {
            removeAllViews()
            choice.options.forEach { option ->
                addView(
                    ChoiceRow(ui, option.title.resolve(bundle.name), option.value == selected)
                        .apply {
                            setOnClickListener {
                                onChoose(option.value)
                                build(option.value)
                            }
                        }
                )
            }
        }
        build(chosen)
    }

private const val OPAQUE = 255
private const val HUES = 360
private const val PERCENT = 100
private const val PREVIEW = 56
