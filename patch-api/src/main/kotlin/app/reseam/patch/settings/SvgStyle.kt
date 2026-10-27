// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import org.w3c.dom.Element

/** The paint an SVG element draws with, inherited down the tree as SVG cascades it. */
internal data class SvgStyle(
    val fill: Paint?,
    val stroke: Paint?,
    val strokeWidth: Float,
    val cap: Cap,
    val join: Join,
    val evenOdd: Boolean,
) {
    fun with(element: Element): SvgStyle {
        fun attribute(name: String) = element.getAttribute(name).takeIf(String::isNotBlank)
        fun paint(name: String, inherited: Paint?): Paint? {
            val value = attribute(name) ?: return inherited
            return paint(value)
        }
        return SvgStyle(
            fill = paint("fill", fill),
            stroke = paint("stroke", stroke),
            strokeWidth =
                attribute("stroke-width")?.let {
                    it.toFloatOrNull()
                        ?: throw IllegalArgumentException("stroke-width=\"$it\" is not a number")
                } ?: strokeWidth,
            cap = attribute("stroke-linecap")?.let { enumOf<Cap>("stroke-linecap", it) } ?: cap,
            join =
                attribute("stroke-linejoin")?.let { enumOf<Join>("stroke-linejoin", it) } ?: join,
            evenOdd =
                attribute("fill-rule")?.let {
                    when (it) {
                        "evenodd" -> true
                        "nonzero" -> false
                        else -> throw IllegalArgumentException("unsupported fill-rule $it")
                    }
                } ?: evenOdd,
        )
    }

    companion object {
        /** SVG's initial values: a black fill and no stroke. */
        val INITIAL =
            SvgStyle(
                fill = Paint.Solid(Color.hex("#000000")),
                stroke = null,
                strokeWidth = 1f,
                cap = Cap.BUTT,
                join = Join.MITER,
                evenOdd = false,
            )

        private fun paint(value: String): Paint? =
            when (value) {
                "none" -> null
                "currentColor" -> Paint.Current
                else -> Paint.Solid(Color.hex(value))
            }

        private inline fun <reified T : Enum<T>> enumOf(attribute: String, value: String): T =
            enumValues<T>().firstOrNull { it.name.equals(value, ignoreCase = true) }
                ?: throw IllegalArgumentException("unsupported $attribute $value")
    }
}
