// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import javax.xml.parsers.DocumentBuilderFactory
import kotlin.test.Test
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.w3c.dom.Element

class LucideTest {
    @Test
    fun `every icon in the shipped sprite reads`() {
        val symbols =
            DocumentBuilderFactory.newInstance()
                .newDocumentBuilder()
                .parse(Lucide::class.java.getResourceAsStream("lucide.svg"))
                .getElementsByTagName("symbol")
        val names =
            (0 until symbols.length).map { (symbols.item(it) as Element).getAttribute("id") }
        assertTrue(names.size > 2000, "the sprite holds the whole set")
        names.forEach { assertTrue(Vector.lucide(it).paths.isNotEmpty(), it) }
    }

    @Test
    fun `an old alias is not an icon`() {
        assertFailsWith<IllegalArgumentException> { Vector.lucide("trash-2") }
    }
}
