// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import kotlin.io.path.createTempDirectory
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.gradle.api.GradleException

class StringResourcesTest {
    private val source =
        """
        <resources xmlns:xliff="urn:oasis:names:tc:xliff:document:1.2">
            <string name="hide_ads">Hide ads</string>
            <string name="proxy" translatable="false">eu.example.dev</string>
            <string name="saved">Saved <xliff:g id="title">%1${'$'}s</xliff:g></string>
        </resources>
        """

    @Test
    fun `translations may leave strings out but not change or add them`() {
        val cases =
            mapOf(
                """<string name="hide_ads">Werbung ausblenden</string>""" to null,
                """<string name="proxy">de.example.dev</string>""" to "translatable=\"false\"",
                """<string name="saved">Gespeichert</string>""" to "other placeholders",
                """<string name="missing">Fehlt</string>""" to "unknown string",
            )
        for ((translation, error) in cases) {
            val root = tables(translation)
            if (error == null) {
                assertEquals(
                    listOf("de", DEFAULT_LOCALE),
                    readStringFiles(root).map { it.locale },
                )
            } else {
                val failure = assertFailsWith<GradleException> { readStringFiles(root) }
                assertTrue(error in failure.message.orEmpty(), "$translation: ${failure.message}")
            }
        }
    }

    private fun tables(translation: String): File =
        createTempDirectory().toFile().apply {
            resolve("values").apply { mkdirs() }.resolve("strings.xml").writeText(source)
            resolve("values-de")
                .apply { mkdirs() }
                .resolve("strings.xml")
                .writeText(
                    """<resources xmlns:xliff="urn:oasis:names:tc:xliff:document:1.2">$translation</resources>"""
                )
        }
}
