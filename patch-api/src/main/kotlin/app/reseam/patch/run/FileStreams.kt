// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.run

import app.reseam.patch.native.fileSourceRange
import app.reseam.patch.native.fileSourceSize
import app.reseam.patch.native.fileStreamClose
import app.reseam.patch.native.fileStreamOpen
import app.reseam.patch.native.fileStreamRead
import app.reseam.patch.native.fileWriteAppend
import app.reseam.patch.native.fileWriteCommit
import app.reseam.patch.native.fileWriteDiscard
import app.reseam.patch.native.fileWriteOpen
import app.reseam.patch.types.FileCompression
import java.io.InputStream

/** The bytes one call into the engine moves. */
private const val CHUNK = 64 * 1024

/**
 * Bytes the engine hands over a chunk at a time, usable only from the patch callback that opened
 * them. The engine drops what is still open when the callback ends.
 */
internal abstract class NativeStream(private val name: String) : InputStream() {
    private val runtime = ActiveRuntime.current
    private var closed = false
    private var buffer = ByteArray(0)
    private var cursor = 0

    /** Up to [length] next bytes; empty only at the end. */
    protected abstract fun fetch(length: Int): ByteArray

    protected open fun release() {}

    protected val buffered: Int
        get() = buffer.size - cursor

    protected fun checkOpen() {
        check(!closed) { "$name is closed" }
        check(ActiveRuntime.current === runtime) { "$name belongs to another callback" }
    }

    /** Forgets the bytes fetched but not read yet, returning how many there were. */
    protected fun dropBuffered(): Int = buffered.also {
        buffer = ByteArray(0)
        cursor = 0
    }

    private fun fill(): Boolean {
        if (buffered > 0) return true
        buffer = fetch(CHUNK)
        cursor = 0
        return buffer.isNotEmpty()
    }

    override fun read(): Int {
        checkOpen()
        if (!fill()) return -1
        return buffer[cursor++].toInt() and 0xff
    }

    override fun read(target: ByteArray, offset: Int, length: Int): Int {
        require(offset >= 0 && length >= 0 && offset <= target.size - length)
        checkOpen()
        if (length == 0) return 0
        if (!fill()) return -1
        val count = minOf(length, buffered)
        buffer.copyInto(target, offset, cursor, cursor + count)
        cursor += count
        return count
    }

    override fun close() {
        if (closed) return
        closed = true
        dropBuffered()
        if (ActiveRuntime.currentOrNull === runtime) release()
    }
}

/** The original APK's bytes, read by offset, so skipping costs nothing. */
internal class ApkSourceStream(private val component: String?) : NativeStream("APK source stream") {
    private val size = fileSourceSize(component).toLong()
    private var offset = 0L

    override fun fetch(length: Int): ByteArray {
        if (offset == size) return ByteArray(0)
        return fileSourceRange(component, offset.toULong(), length.toUInt()).also {
            check(it.isNotEmpty()) { "APK source changed while reading" }
            offset += it.size
        }
    }

    override fun skip(count: Long): Long {
        checkOpen()
        val position = offset - dropBuffered()
        val skipped = count.coerceIn(0, size - position)
        offset = position + skipped
        return skipped
    }

    override fun available(): Int {
        checkOpen()
        return (size - offset + buffered).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
    }
}

/** One file of the APK as it was when opened, decoded by the engine as it is read. */
internal class EntryStream private constructor(private val handle: UInt) :
    NativeStream("file stream") {
    override fun fetch(length: Int): ByteArray = fileStreamRead(handle, length.toUInt())

    override fun release() = fileStreamClose(handle)

    companion object {
        fun open(component: String?, path: String): EntryStream? =
            fileStreamOpen(component, path)?.let(::EntryStream)
    }
}

/**
 * Adds or replaces [path] with everything [data] reads, a chunk at a time. Nothing changes when
 * reading [data] fails. [data] stays open.
 */
internal fun writeStream(
    component: String?,
    path: String,
    data: InputStream,
    compression: FileCompression,
) {
    val handle = fileWriteOpen(component, path, compression)
    try {
        val chunk = ByteArray(CHUNK)
        do {
            val count = data.readInto(chunk)
            if (count > 0)
                fileWriteAppend(handle, if (count == chunk.size) chunk else chunk.copyOf(count))
        } while (count == chunk.size)
    } catch (error: Throwable) {
        runCatching { fileWriteDiscard(handle) }.exceptionOrNull()?.let(error::addSuppressed)
        throw error
    }
    fileWriteCommit(handle)
}

/** Fills [target] unless the stream ends first, returning how much it read. */
private fun InputStream.readInto(target: ByteArray): Int {
    var count = 0
    while (count < target.size) {
        val read = read(target, count, target.size - count)
        if (read < 0) break
        count += read
    }
    return count
}
