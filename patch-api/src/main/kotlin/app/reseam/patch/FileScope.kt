// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.componentNames
import app.reseam.patch.native.fileCopy
import app.reseam.patch.native.fileDelete
import app.reseam.patch.native.fileInject
import app.reseam.patch.native.fileList
import app.reseam.patch.native.fileRead
import app.reseam.patch.native.fileSigners
import app.reseam.patch.native.fileSourceRange
import app.reseam.patch.native.fileSourceSize
import app.reseam.patch.native.fileXmlWith
import app.reseam.patch.native.xmlOpen
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.types.FileCompression
import java.io.InputStream

class FileScope internal constructor(private val componentName: String? = null) {
    fun components(): List<String> = componentNames()

    fun component(name: String): FileScope = FileScope(name)

    fun list(): List<String> = fileList(componentName)

    fun read(path: String): ByteArray? = fileRead(componentName, path)

    /**
     * Compiled XML files where an element carries one of [attributes], named as [XmlElement.get]
     * takes them.
     */
    fun xmlFilesWith(vararg attributes: String): List<String> =
        fileXmlWith(componentName, attributes.toList())

    /**
     * Streams original APK bytes, including the signing block, in bounded reads. The stream is
     * confined to the current patch callback and must be closed before that callback returns.
     * Staged edits do not affect it. Native read failures are propagated to the caller.
     */
    fun sourceStream(): InputStream =
        ApkSourceStream(componentName, fileSourceSize(componentName).toLong())

    /**
     * The original signer certificates, DER-encoded X.509: v3 signers when present, otherwise v2;
     * empty when unsigned or v1-only.
     */
    fun signers(): List<ByteArray> = fileSigners(componentName)

    fun write(path: String, data: ByteArray) =
        fileInject(componentName, path, data, FileCompression.DEFLATED)

    fun writeStored(path: String, data: ByteArray) =
        fileInject(componentName, path, data, FileCompression.STORED)

    fun delete(path: String) = fileDelete(componentName, path)

    /** Copies a file shipped in the bundle into the APK. */
    fun copy(bundlePath: String, apkPath: String) = fileCopy(componentName, bundlePath, apkPath)

    fun xml(path: String): XmlDocument =
        XmlDocument(xmlOpen(componentName, path) ?: error("failed to open XML document: $path"))

    fun <T> editXml(path: String, block: XmlDocument.() -> T): T = xml(path).use(block)
}

private class ApkSourceStream(private val component: String?, private val size: Long) :
    InputStream() {
    private val runtime = ActiveRuntime.current
    private var position = 0L
    private var closed = false
    private var buffer = ByteArray(0)
    private var cursor = 0

    override fun read(): Int {
        check(!closed) { "APK source stream is closed" }
        check(ActiveRuntime.current === runtime) { "APK source stream belongs to another callback" }
        if (cursor == buffer.size) {
            if (position == size) return -1
            buffer = fileSourceRange(component, position.toULong(), 65536u)
            check(buffer.isNotEmpty()) { "APK source changed while reading" }
            cursor = 0
        }
        position++
        return buffer[cursor++].toInt() and 0xff
    }

    override fun read(target: ByteArray, offset: Int, length: Int): Int {
        require(offset >= 0 && length >= 0 && offset <= target.size - length)
        check(!closed) { "APK source stream is closed" }
        check(ActiveRuntime.current === runtime) { "APK source stream belongs to another callback" }
        if (length == 0) return 0
        if (position == size) return -1
        if (cursor == buffer.size) {
            buffer = fileSourceRange(component, position.toULong(), minOf(length, 65536).toUInt())
            check(buffer.isNotEmpty()) { "APK source changed while reading" }
            cursor = 0
        }
        val count = minOf(length, buffer.size - cursor)
        buffer.copyInto(target, offset, cursor, cursor + count)
        cursor += count
        position += count
        return count
    }

    override fun skip(count: Long): Long {
        check(!closed) { "APK source stream is closed" }
        check(ActiveRuntime.current === runtime) { "APK source stream belongs to another callback" }
        val skipped = count.coerceIn(0, size - position)
        position += skipped
        buffer = ByteArray(0)
        cursor = 0
        return skipped
    }

    override fun available(): Int {
        check(!closed) { "APK source stream is closed" }
        check(ActiveRuntime.current === runtime) { "APK source stream belongs to another callback" }
        return (size - position).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
    }

    override fun close() {
        closed = true
        buffer = ByteArray(0)
        cursor = 0
    }
}
