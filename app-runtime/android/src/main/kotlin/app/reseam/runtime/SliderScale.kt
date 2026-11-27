// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import java.math.BigDecimal
import java.math.RoundingMode
import kotlin.math.log10
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.roundToLong

internal class SliderScale(private val slider: Slider) {
    private val decimals = decimals(slider.step)
    private val shownDecimals = decimals(slider.step * slider.scale)

    private val magnitudes = log10((slider.max - slider.min) / slider.step)

    val steps: Int =
        if (slider.logarithmic) LOGARITHMIC_STEPS
        else ((slider.max - slider.min) / slider.step).roundToInt()

    fun valueAt(position: Int): Double {
        val (min, max) = slider.min to slider.max
        val fraction = position.toDouble() / steps
        val raw =
            if (slider.logarithmic)
                min +
                    (max - min) * (10.0.pow(magnitudes * fraction) - 1) / (10.0.pow(magnitudes) - 1)
            else min + (max - min) * fraction
        val stepped = min + ((raw - min) / slider.step).roundToLong() * slider.step
        return round(stepped.coerceIn(min, max))
    }

    fun positionOf(value: Double): Int {
        val fraction = ((value - slider.min) / (slider.max - slider.min)).coerceIn(0.0, 1.0)
        val scaled =
            if (slider.logarithmic) log10(1 + fraction * (10.0.pow(magnitudes) - 1)) / magnitudes
            else fraction
        return (scaled * steps).roundToInt()
    }

    fun covers(value: Double) = value in slider.min..slider.max

    fun covers(range: NumberRange) = covers(range.start) && covers(range.end)

    fun label(value: Double, bundle: String): CharSequence {
        val end =
            if (value <= slider.min) slider.minLabel
            else if (value >= slider.max) slider.maxLabel else null
        return end?.resolve(bundle)
            ?: (BigDecimal(value * slider.scale)
                .setScale(shownDecimals, RoundingMode.HALF_UP)
                .toPlainString() + slider.unit)
    }

    fun round(value: Double): Double =
        BigDecimal(value).setScale(decimals, RoundingMode.HALF_UP).toDouble()
}

internal object NumberText {
    fun format(value: Double): String =
        BigDecimal.valueOf(value).stripTrailingZeros().toPlainString()

    fun parse(text: String): Double? = text.trim().toDoubleOrNull()?.takeIf { it.isFinite() }

    fun format(range: NumberRange): String = "${format(range.start)}, ${format(range.end)}"

    fun parseRange(text: String): NumberRange? {
        val parts = text.split(",")
        if (parts.size != 2) return null
        val start = parse(parts[0]) ?: return null
        val end = parse(parts[1]) ?: return null
        return if (start <= end) NumberRange(start, end) else null
    }
}

private fun decimals(number: Double) =
    BigDecimal(number.toString()).stripTrailingZeros().scale().coerceAtLeast(0)

private const val LOGARITHMIC_STEPS = 1000
