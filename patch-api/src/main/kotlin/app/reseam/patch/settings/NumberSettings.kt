// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

/**
 * Numbers from [min] to [max] in steps of [step], shown times [scale] with [unit] after them, or as
 * [minLabel] and [maxLabel] at the ends. A [logarithmic] slider spreads its steps over orders of
 * magnitude, for ranges such as view counts. A [dialog] slider shows only the summary in its row,
 * and a tap opens the slider in a dialog.
 */
data class Slider(
    val min: Double,
    val max: Double,
    val step: Double,
    val unit: String = "",
    val logarithmic: Boolean = false,
    val scale: Double = 1.0,
    val minLabel: StringRef? = null,
    val maxLabel: StringRef? = null,
    val dialog: Boolean = false,
) {
    init {
        require(min.isFinite() && max.isFinite() && step.isFinite() && min < max && step > 0) {
            "A slider needs min < max and a positive step, got $min..$max by $step"
        }
        require(!logarithmic || min >= 0) { "A logarithmic slider cannot go below zero" }
        require(scale.isFinite() && scale > 0) { "A slider's scale must be positive, got $scale" }
    }

    internal fun covers(value: Double) = value in min..max
}

class NumberSetting
internal constructor(
    key: String,
    internal override val property: String,
    override val title: StringRef,
    summary: StringRef?,
    default: Double,
    icon: Vector?,
    takesEffect: TakesEffect,
    val slider: Slider,
) : Setting<Double>(SettingKind.NUMBER, key, property, title, summary, default, icon, takesEffect) {
    init {
        require(slider.covers(default)) {
            "Number setting '$key' defaults to $default, off its slider"
        }
    }
}

class RangeSetting
internal constructor(
    key: String,
    internal override val property: String,
    override val title: StringRef,
    summary: StringRef?,
    default: ClosedFloatingPointRange<Double>,
    icon: Vector?,
    takesEffect: TakesEffect,
    val slider: Slider,
) :
    Setting<ClosedFloatingPointRange<Double>>(
        SettingKind.RANGE,
        key,
        property,
        title,
        summary,
        default,
        icon,
        takesEffect,
    ) {
    init {
        require(
            slider.covers(default.start) &&
                slider.covers(default.endInclusive) &&
                default.start <= default.endInclusive
        ) {
            "Range setting '$key' defaults to $default, off its slider"
        }
    }
}
