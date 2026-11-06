// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import javax.xml.parsers.DocumentBuilderFactory
import org.w3c.dom.Element

/** Lucide's icons, from the sprite of the Lucide release the patch API ships (`lucide.svg`). */
internal object Lucide {
    /**
     * How Lucide draws every icon. The sprite leaves paint to the `<svg>` that shows a symbol, and
     * Lucide's single-icon files carry these attributes.
     */
    private val PAINT =
        SvgStyle(
            fill = null,
            stroke = Paint.Current,
            strokeWidth = 2f,
            cap = Cap.ROUND,
            join = Join.ROUND,
            evenOdd = false,
        )

    private val symbols: Map<String, Element> by lazy {
        val sprite =
            Lucide::class.java.getResourceAsStream("lucide.svg").use {
                DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(it)
            }
        val list = sprite.getElementsByTagName("symbol")
        (0 until list.length).map { list.item(it) as Element }.associateBy { it.getAttribute("id") }
    }

    @Synchronized
    fun icon(name: String): Vector =
        SvgReader.symbol(
            symbols[name] ?: throw IllegalArgumentException("Lucide has no icon named \"$name\""),
            PAINT,
        )
}
