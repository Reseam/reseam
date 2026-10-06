// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.native.componentNames
import app.reseam.patch.native.fileCopy
import app.reseam.patch.native.fileDelete
import app.reseam.patch.native.fileInject
import app.reseam.patch.native.fileList
import app.reseam.patch.native.fileRead
import app.reseam.patch.native.fileSigners
import app.reseam.patch.native.fileSource
import app.reseam.patch.native.fileSourceRange
import app.reseam.patch.native.fileSourceSize
import app.reseam.patch.native.findInstructionsByString
import app.reseam.patch.native.findInstructionsByStringContains
import app.reseam.patch.native.logDebug
import app.reseam.patch.native.logInfo
import app.reseam.patch.native.logWarn
import app.reseam.patch.native.xmlOpen
import app.reseam.patch.types.MethodRef
import java.io.InputStream

class PatchLogger internal constructor() {
    fun info(message: String) = logInfo(message)

    fun warn(message: String) = logWarn(message)

    fun debug(message: String) = logDebug(message)
}

class BytecodeScope internal constructor() {
    val classes: List<DexClass>
        get() = ActiveRuntime.current.index.allClasses

    fun findClass(name: String): DexClass? = ActiveRuntime.current.index.classFor(descriptor(name))

    fun classesExtending(type: String): List<DexClass> {
        val wanted = descriptor(type)
        return classes.filter { ActiveRuntime.current.index.classExtends(it, wanted) }
    }

    /**
     * Rewrites every `const-string` equal to `old` in the app; returns how many constants changed.
     */
    fun replaceAllStrings(old: String, new: String): Int =
        findInstructionsByString(old)
            .map { Method(it.method) }
            .distinctBy { it.handle }
            .sumOf { it.replaceAllStrings(old, new) }

    /**
     * Rewrites every `const-string` containing [substring] through [transform], which returns the
     * replacement or null to leave that constant as it is. Returns how many constants changed.
     *
     * Seeded from the string index, so only methods holding a matching constant are visited. Use it
     * when a value is embedded in larger constants rather than stored on its own, such as an
     * authority inside a `content://` URI; [replaceAllStrings] covers the whole-string case.
     */
    fun replaceStringsContaining(substring: String, transform: (String) -> String?): Int =
        findInstructionsByStringContains(substring)
            .mapNotNull { Method(it.method).stringRef(it.index.toInt()) }
            .distinct()
            .sumOf { old ->
                transform(old)?.takeIf { it != old }?.let { replaceAllStrings(old, it) } ?: 0
            }

    /**
     * Every call to `from` in the app becomes a call to the static `to`, receiver first for
     * instance methods. Calls through `super`, constructor calls, and calls from extensions are
     * left alone, so `to` can call `from`. Returns how many call sites changed.
     */
    fun redirectCalls(from: MethodRef, to: ExtMethod): Int {
        val callers =
            app.reseam.patch.methods("callers of ${from.descriptor}") {
                calls(from)
            }
        return callers
            .points("redirect ${from.descriptor}") {
                invoke(*redirectableInvokes.toTypedArray()) {
                    owner(from.definingClass)
                    name(from.name)
                    params(*from.parameterTypes.toTypedArray())
                    returns(from.returnType)
                }
            }
            .all
            .redirectTo(to)
    }

    /**
     * [redirectCalls] with `from` derived from `to`: `to` takes the same parameters as
     * `owner.name`, preceded by the receiver when `owner.name` is an instance method, and returns
     * the same type.
     */
    fun redirectCalls(owner: String, name: String, to: ExtMethod): Int {
        val ownerDesc = descriptor(owner)
        val params = to.ref.parameterTypes
        val fromParams = if (params.firstOrNull() == ownerDesc) params.drop(1) else params
        return redirectCalls(
            MethodRef(ownerDesc, name, proto(to.ref.returnType, *fromParams.toTypedArray())),
            to,
        )
    }
}

class FileScope internal constructor(private val componentName: String? = null) {
    fun components(): List<String> = componentNames()

    fun component(name: String): FileScope = FileScope(name)

    fun list(): List<String> = fileList(componentName)

    fun read(path: String): ByteArray? = fileRead(componentName, path)

    /** The original, unmodified bytes of this component's APK, signing block included. */
    @Deprecated("Use sourceStream() to avoid copying the entire APK into memory")
    fun source(): ByteArray? = fileSource(componentName)

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

    fun write(path: String, data: ByteArray) = fileInject(componentName, path, data, false)

    fun writeStored(path: String, data: ByteArray) = fileInject(componentName, path, data, true)

    fun delete(path: String) = fileDelete(componentName, path)

    /** Copies a file shipped in the bundle into the APK. */
    fun copy(bundlePath: String, apkPath: String) = fileCopy(componentName, bundlePath, apkPath)

    fun xml(path: String): XmlDocument =
        XmlDocument(xmlOpen(componentName, path) ?: error("failed to open XML document: $path"))

    fun <T> editXml(path: String, block: XmlDocument.() -> T): T = xml(path).use(block)
}

/** A manifest attribute such as `@0x7f1400a0` as a resource id. */
fun resourceRef(value: String): UInt? =
    when {
        value.startsWith("@0x") -> value.removePrefix("@0x").toUIntOrNull(16)
        value.startsWith("@ref/0x") -> value.removePrefix("@ref/0x").toUIntOrNull(16)
        else -> null
    }

/** An Android resource scalar. [kind] is the Android type byte and [data] its unmodified bits. */
data class ResourceValue(val kind: UByte, val data: UInt)

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
