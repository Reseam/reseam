// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import javax.xml.parsers.DocumentBuilderFactory
import org.w3c.dom.Element
import org.xml.sax.InputSource

internal object SvgReader {
    private val SHAPES = setOf("path", "circle", "ellipse", "rect", "line", "polyline", "polygon")
    private val PAINT =
        setOf("fill", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin", "fill-rule")
    private val GEOMETRY =
        mapOf(
            "svg" to setOf("xmlns", "width", "height", "viewBox", "class"),
            "g" to setOf("class"),
            "path" to setOf("d"),
            "circle" to setOf("cx", "cy", "r"),
            "ellipse" to setOf("cx", "cy", "rx", "ry"),
            "rect" to setOf("x", "y", "width", "height", "rx", "ry"),
            "line" to setOf("x1", "y1", "x2", "y2"),
            "polyline" to setOf("points"),
            "polygon" to setOf("points"),
        )

    fun read(source: String): Vector {
        val root =
            DocumentBuilderFactory.newInstance()
                .newDocumentBuilder()
                .parse(InputSource(source.reader()))
                .documentElement
        require(root.tagName == "svg") {
            "an SVG document starts with <svg>, not <${root.tagName}>"
        }
        val box =
            root.getAttribute("viewBox").takeIf(String::isNotBlank)?.let(::numbers)
                ?: listOf(0f, 0f, root.number("width"), root.number("height"))
        require(box.size == 4 && box[2] > 0 && box[3] > 0) {
            "viewBox needs a positive width and height"
        }
        val paths = mutableListOf<VectorPath>()
        collect(root, SvgStyle.INITIAL, PathNormalizer(-box[0], -box[1]), paths)
        require(paths.isNotEmpty()) { "the SVG draws nothing" }
        return Vector(box[2], box[3], paths)
    }

    private fun collect(
        element: Element,
        inherited: SvgStyle,
        normalizer: PathNormalizer,
        paths: MutableList<VectorPath>,
    ) {
        val name = element.tagName
        val geometry = GEOMETRY[name] ?: throw IllegalArgumentException("unsupported SVG <$name>")
        val attributes = element.attributes
        for (index in 0 until attributes.length) {
            val attribute = attributes.item(index).nodeName
            require(attribute in geometry || attribute in PAINT || attribute.startsWith("xmlns:")) {
                "unsupported attribute $attribute on SVG <$name>"
            }
        }
        val style = inherited.with(element)
        if (name in SHAPES) {
            val data = normalizer.normalize(outline(element))
            val fill = style.fill?.let { Fill(it, style.evenOdd) }
            val stroke = style.stroke?.let { Stroke(it, style.strokeWidth, style.cap, style.join) }
            if (fill != null || stroke != null) paths += VectorPath(data, fill, stroke)
        }
        val children = element.childNodes
        for (index in 0 until children.length) {
            (children.item(index) as? Element)?.let { collect(it, style, normalizer, paths) }
        }
    }

    private fun outline(element: Element): String =
        when (element.tagName) {
            "path" -> element.getAttribute("d")
            "circle" -> ellipse(element.number("cx"), element.number("cy"), element.number("r"))
            "ellipse" ->
                ellipse(
                    element.number("cx"),
                    element.number("cy"),
                    element.number("rx"),
                    element.number("ry"),
                )
            "rect" -> rect(element)
            "line" ->
                "M ${element.number("x1")} ${element.number("y1")} " +
                    "L ${element.number("x2")} ${element.number("y2")}"
            "polyline" -> points(element)
            "polygon" -> points(element) + " Z"
            else -> ""
        }

    private fun ellipse(cx: Float, cy: Float, rx: Float, ry: Float = rx) =
        "M ${cx - rx} $cy A $rx $ry 0 1 0 ${cx + rx} $cy A $rx $ry 0 1 0 ${cx - rx} $cy Z"

    private fun rect(element: Element): String {
        val x = element.number("x")
        val y = element.number("y")
        val width = element.number("width")
        val height = element.number("height")
        val rxAttribute = element.getAttribute("rx").takeIf(String::isNotBlank)?.toFloat()
        val ryAttribute = element.getAttribute("ry").takeIf(String::isNotBlank)?.toFloat()
        val rx = (rxAttribute ?: ryAttribute ?: 0f).coerceAtMost(width / 2)
        val ry = (ryAttribute ?: rxAttribute ?: 0f).coerceAtMost(height / 2)
        if (rx == 0f || ry == 0f) return "M $x $y H ${x + width} V ${y + height} H $x Z"
        return "M ${x + rx} $y H ${x + width - rx} A $rx $ry 0 0 1 ${x + width} ${y + ry} " +
            "V ${y + height - ry} A $rx $ry 0 0 1 ${x + width - rx} ${y + height} " +
            "H ${x + rx} A $rx $ry 0 0 1 $x ${y + height - ry} V ${y + ry} " +
            "A $rx $ry 0 0 1 ${x + rx} $y Z"
    }

    private fun points(element: Element): String {
        val values = numbers(element.getAttribute("points"))
        require(values.size >= 4 && values.size % 2 == 0) {
            "<${element.tagName}> needs at least two x,y points"
        }
        return values.chunked(2).withIndex().joinToString(" ") { (index, point) ->
            "${if (index == 0) "M" else "L"} ${point[0]} ${point[1]}"
        }
    }

    private fun Element.number(name: String): Float {
        val value = getAttribute(name)
        if (value.isBlank()) return 0f
        return value.removeSuffix("px").toFloatOrNull()
            ?: throw IllegalArgumentException("$name=\"$value\" on SVG <$tagName> is not a number")
    }

    private fun numbers(value: String): List<Float> =
        value.trim().split(Regex("[\\s,]+")).filter(String::isNotEmpty).map {
            it.toFloatOrNull() ?: throw IllegalArgumentException("\"$it\" is not a number")
        }
}
