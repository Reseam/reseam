// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

/**
 * A vector image, such as an icon or a logo, that the app runtime draws at any size. Read one from
 * an SVG with [svg], or take a Lucide icon with [lucide].
 */
class Vector
internal constructor(
    internal val width: Float,
    internal val height: Float,
    internal val paths: List<VectorPath>,
) {
    companion object {
        /**
         * Reads an SVG document. `path`, `circle`, `ellipse`, `rect`, `line`, `polyline` and
         * `polygon` draw, inside any number of `g`. Paint comes from the `fill`, `stroke`,
         * `stroke-width`, `stroke-linecap`, `stroke-linejoin` and `fill-rule` attributes, inherited
         * from enclosing elements: `currentColor` takes the color the screen draws the icon in, a
         * `#rgb` or `#rrggbb` color stays fixed, and `none` paints nothing. Anything else, such as
         * transforms, `style`, gradients or text, fails.
         *
         * ```kotlin
         * val icon = Vector.svg(Icons::class.java.getResource("/icons/layers.svg")!!.readText())
         * ```
         */
        fun svg(source: String): Vector = SvgReader.read(source)

        /**
         * The [Lucide](https://lucide.dev/icons) icon [name], such as `"trash"`, drawn as Lucide
         * draws it: a 2-wide round stroke in the screen's icon color. The patch API ships the whole
         * set of Lucide 1.66.0, so a bundle carries no icon files. Use the icon's current name;
         * Lucide's old aliases, such as `"trash-2"`, fail.
         */
        fun lucide(name: String): Vector = Lucide.icon(name)
    }
}

internal class VectorPath(
    /** Absolute `M`, `L`, `C` and `Z` commands. */
    val data: String,
    val fill: Fill?,
    val stroke: Stroke?,
)

internal class Fill(val paint: Paint, val evenOdd: Boolean)

internal class Stroke(val paint: Paint, val width: Float, val cap: Cap, val join: Join)

internal sealed interface Paint {
    /** The color the screen draws the vector in. */
    data object Current : Paint

    data class Solid(val color: Color) : Paint
}

internal enum class Cap {
    BUTT,
    ROUND,
    SQUARE,
}

internal enum class Join {
    MITER,
    ROUND,
    BEVEL,
}
