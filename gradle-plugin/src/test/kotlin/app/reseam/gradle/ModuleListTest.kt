// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals

class ModuleListTest {
    @Test
    fun `the module list round trips the fixture the engine reads`() {
        val fixture = File(System.getProperty("reseam.moduleListFixture")).readText().trim()
        assertEquals(
            fixture,
            bundleJson.encodeToString(bundleJson.decodeFromString<List<ModuleListEntry>>(fixture)),
        )
    }
}
