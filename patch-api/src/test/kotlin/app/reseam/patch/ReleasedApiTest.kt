// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * Bundles load on every engine of their major line and link against its patch API by JVM signature,
 * so each class and member of the last release must stay.
 */
class ReleasedApiTest {
    @Test
    fun `every class and member of the released patch API remains`() {
        val current = signatures(File("api/reseam-patch-sdk.api"))
        val missing = signatures(File("api/released.api")) - current
        assertEquals(emptySet(), missing, "released bundles would fail to link to these")
    }

    private fun signatures(dump: File): Set<String> {
        var owner = ""
        return dump
            .readLines()
            .mapNotNull { line ->
                when {
                    line.endsWith("{") && !line.startsWith("\t") -> {
                        owner =
                            line.substringBefore(" : ").removeSuffix(" {").substringAfterLast(' ')
                        owner
                    }
                    line.startsWith("\t") ->
                        MEMBER.find(line)?.let { "$owner ${it.groupValues[1]}" }
                    else -> null
                }
            }
            .toSet()
    }
}

private val MEMBER = Regex("""(?:fun|field) (\S+ .*)$""")
