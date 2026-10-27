// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals

class PatchIndexTest {
    @Test
    fun `the patch index round trips the fixture the engine reads`() {
        val fixture = File(System.getProperty("reseam.patchIndexFixture")).readText().trim()
        assertEquals(
            fixture,
            bundleJson.encodeToString(bundleJson.decodeFromString<List<PatchDeclaration>>(fixture)),
        )
    }
}
