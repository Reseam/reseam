// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import java.io.IOException
import java.io.InputStream

/** Larger than the chunks streams move, so reads and writes take several. */
private val LARGE = ByteArray(3 * 1024 * 1024 + 17) { ((it % 251) xor (it / 4093)).toByte() }

/** Fails after [length] bytes, as a source that breaks partway does. */
private class FailingStream(private var length: Int) : InputStream() {
    override fun read(): Int = if (length-- > 0) 1 else throw IOException("source failed")
}

val fileStreams =
    fixturePatch("file-streams") {
        files.write("assets/large.bin", LARGE.inputStream())
        files.writeStored("assets/large-stored.bin", LARGE.inputStream())
        files.write("res/xml/streamed.xml", "<settings />".byteInputStream())

        val opened = checkNotNull(files.stream("assets/large.bin"))
        files.write("assets/large.bin", "replaced".encodeToByteArray())
        check(opened.use { it.readBytes() }.contentEquals(LARGE)) {
            "a stream reads the file it opened"
        }
        check(
            files.stream("assets/large.bin")!!.use { it.readBytes() }.decodeToString() == "replaced"
        )
        check(files.stream("assets/missing.bin") == null)
        check(
            files
                .stream("AndroidManifest.xml")!!
                .use { it.readBytes() }
                .contentEquals(files.read("AndroidManifest.xml"))
        )

        val closed = files.stream("assets/large-stored.bin")!!.apply { close() }
        check(runCatching { closed.read() }.isFailure) { "a closed stream reads nothing" }

        check(runCatching { files.write("assets/failed.bin", FailingStream(100_000)) }.isFailure)
        check(files.read("assets/failed.bin") == null) { "a failed write leaves nothing" }
    }
