// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import kotlin.test.Test
import kotlin.test.assertEquals

class IntegrationTest {

    @Test
    fun `JNI round-trip works`() {
        val v = version()
        assertEquals("0.1.0", v)
    }
}
