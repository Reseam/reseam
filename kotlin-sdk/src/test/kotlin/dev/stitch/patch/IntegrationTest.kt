package dev.stitch.patch

import kotlin.test.Test
import kotlin.test.assertEquals

class IntegrationTest {

    @Test
    fun `JNI round-trip works`() {
        val v = version()
        assertEquals("0.1.0", v)
    }
}
