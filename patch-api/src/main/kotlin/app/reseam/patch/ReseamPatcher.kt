// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:OptIn(kotlin.ExperimentalUnsignedTypes::class)

package app.reseam.patch

private object Utf8Codec {
    fun maxBytes(value: String): Int = value.length * 3
}

private val <First, Second> Pair<First, Second>.field0: First get() = first
private val <First, Second> Pair<First, Second>.field1: Second get() = second
private val <First, Second, Third> Triple<First, Second, Third>.field0: First get() = first
private val <First, Second, Third> Triple<First, Second, Third>.field1: Second get() = second
private val <First, Second, Third> Triple<First, Second, Third>.field2: Third get() = third

class FfiException(message: String) : RuntimeException(message)

internal class BoltFfiErrorBufferException(val bytes: ByteArray) : RuntimeException("BoltFFI call failed")

private object DirectVectorCodec {
    fun readBooleanArray(bytes: ByteArray): BooleanArray =
        BooleanArray(bytes.size) { index -> bytes[index] != 0.toByte() }

    fun readByteArray(bytes: ByteArray): ByteArray = bytes

    fun writeBooleanArray(values: BooleanArray): ByteArray =
        ByteArray(values.size) { index -> if (values[index]) 1.toByte() else 0.toByte() }

    fun writeByteArray(values: ByteArray): ByteArray = values

    fun <T> readRecordList(
        bytes: ByteArray,
        width: Int,
        read: (java.nio.ByteBuffer, Int) -> T
    ): List<T> {
        val count = elementCount(bytes, width)
        val buffer = nativeBuffer(bytes)
        return List(count) { index -> read(buffer, index * width) }
    }

    fun <T> writeRecordList(
        values: List<T>,
        width: Int,
        write: (T, java.nio.ByteBuffer, Int) -> Unit
    ): ByteArray {
        val bytes = ByteArray(values.size * width)
        val buffer = nativeBuffer(bytes)
        values.forEachIndexed { index, value -> write(value, buffer, index * width) }
        return bytes
    }

    fun readShortArray(bytes: ByteArray): ShortArray {
        val values = ShortArray(elementCount(bytes, 2))
        nativeBuffer(bytes).asShortBuffer().get(values)
        return values
    }

    fun readUShortArray(bytes: ByteArray): UShortArray =
        readShortArray(bytes).toUShortArray()

    fun writeShortArray(values: ShortArray): ByteArray {
        val bytes = ByteArray(values.size * 2)
        nativeBuffer(bytes).asShortBuffer().put(values)
        return bytes
    }

    fun writeUShortArray(values: UShortArray): ByteArray =
        writeShortArray(values.asShortArray())

    fun readIntArray(bytes: ByteArray): IntArray {
        val values = IntArray(elementCount(bytes, 4))
        nativeBuffer(bytes).asIntBuffer().get(values)
        return values
    }

    fun readUIntArray(bytes: ByteArray): UIntArray =
        readIntArray(bytes).toUIntArray()

    fun writeIntArray(values: IntArray): ByteArray {
        val bytes = ByteArray(values.size * 4)
        nativeBuffer(bytes).asIntBuffer().put(values)
        return bytes
    }

    fun writeUIntArray(values: UIntArray): ByteArray =
        writeIntArray(values.asIntArray())

    fun readLongArray(bytes: ByteArray): LongArray {
        val values = LongArray(elementCount(bytes, 8))
        nativeBuffer(bytes).asLongBuffer().get(values)
        return values
    }

    fun readULongArray(bytes: ByteArray): ULongArray =
        readLongArray(bytes).toULongArray()

    fun writeLongArray(values: LongArray): ByteArray {
        val bytes = ByteArray(values.size * 8)
        nativeBuffer(bytes).asLongBuffer().put(values)
        return bytes
    }

    fun writeULongArray(values: ULongArray): ByteArray =
        writeLongArray(values.asLongArray())

    fun readFloatArray(bytes: ByteArray): FloatArray {
        val values = FloatArray(elementCount(bytes, 4))
        nativeBuffer(bytes).asFloatBuffer().get(values)
        return values
    }

    fun writeFloatArray(values: FloatArray): ByteArray {
        val bytes = ByteArray(values.size * 4)
        nativeBuffer(bytes).asFloatBuffer().put(values)
        return bytes
    }

    fun readDoubleArray(bytes: ByteArray): DoubleArray {
        val values = DoubleArray(elementCount(bytes, 8))
        nativeBuffer(bytes).asDoubleBuffer().get(values)
        return values
    }

    fun writeDoubleArray(values: DoubleArray): ByteArray {
        val bytes = ByteArray(values.size * 8)
        nativeBuffer(bytes).asDoubleBuffer().put(values)
        return bytes
    }

    private fun nativeBuffer(bytes: ByteArray): java.nio.ByteBuffer =
        java.nio.ByteBuffer
            .wrap(bytes)
            .order(java.nio.ByteOrder.nativeOrder())

    private fun elementCount(bytes: ByteArray, width: Int): Int {
        require(bytes.size % width == 0)
        return bytes.size / width
    }
}

internal class WireReader(private val bytes: ByteArray) {
    private var position = 0

    fun readBool(): Boolean = readI8() != 0.toByte()

    fun readI8(): Byte {
        val value = bytes[position]
        position += 1
        return value
    }

    fun readU8(): UByte = readI8().toUByte()

    fun readI16(): Short {
        val value =
            (bytes[position].toInt() and 0xff) or
                ((bytes[position + 1].toInt() and 0xff) shl 8)
        position += 2
        return value.toShort()
    }

    fun readU16(): UShort = readI16().toUShort()

    fun readI32(): Int {
        val value =
            (bytes[position].toInt() and 0xff) or
                ((bytes[position + 1].toInt() and 0xff) shl 8) or
                ((bytes[position + 2].toInt() and 0xff) shl 16) or
                ((bytes[position + 3].toInt() and 0xff) shl 24)
        position += 4
        return value
    }

    fun readU32(): UInt = readI32().toUInt()

    fun readI64(): Long {
        val low = readI32().toLong() and 0xffffffffL
        val high = readI32().toLong() and 0xffffffffL
        return low or (high shl 32)
    }

    fun readU64(): ULong = readI64().toULong()

    fun readF32(): Float = java.lang.Float.intBitsToFloat(readI32())

    fun readF64(): Double = java.lang.Double.longBitsToDouble(readI64())

    fun readOptionalBool(): Boolean? = readOptional { it.readBool() }

    fun readOptionalI8(): Byte? = readOptional { it.readI8() }

    fun readOptionalU8(): UByte? = readOptional { it.readU8() }

    fun readOptionalI16(): Short? = readOptional { it.readI16() }

    fun readOptionalU16(): UShort? = readOptional { it.readU16() }

    fun readOptionalI32(): Int? = readOptional { it.readI32() }

    fun readOptionalU32(): UInt? = readOptional { it.readU32() }

    fun readOptionalI64(): Long? = readOptional { it.readI64() }

    fun readOptionalU64(): ULong? = readOptional { it.readU64() }

    fun readOptionalF32(): Float? = readOptional { it.readF32() }

    fun readOptionalF64(): Double? = readOptional { it.readF64() }

    fun readString(): String {
        val length = readU32().toInt()
        val value = String(bytes, position, length, Charsets.UTF_8)
        position += length
        return value
    }

    fun readBytes(): ByteArray {
        val length = readU32().toInt()
        val value = bytes.copyOfRange(position, position + length)
        position += length
        return value
    }

    fun readBooleanArray(): BooleanArray {
        val length = readU32().toInt()
        return BooleanArray(length) { readBool() }
    }

    fun readByteArray(): ByteArray = readBytes()

    fun readShortArray(): ShortArray {
        val length = readU32().toInt()
        val byteCount = length * 2
        val values = ShortArray(length)
        java.nio.ByteBuffer
            .wrap(bytes, position, byteCount)
            .order(java.nio.ByteOrder.LITTLE_ENDIAN)
            .asShortBuffer()
            .get(values)
        position += byteCount
        return values
    }

    fun readUShortArray(): UShortArray =
        readShortArray().toUShortArray()

    fun readIntArray(): IntArray {
        val length = readU32().toInt()
        val byteCount = length * 4
        val values = IntArray(length)
        java.nio.ByteBuffer
            .wrap(bytes, position, byteCount)
            .order(java.nio.ByteOrder.LITTLE_ENDIAN)
            .asIntBuffer()
            .get(values)
        position += byteCount
        return values
    }

    fun readUIntArray(): UIntArray =
        readIntArray().toUIntArray()

    fun readLongArray(): LongArray {
        val length = readU32().toInt()
        val byteCount = length * 8
        val values = LongArray(length)
        java.nio.ByteBuffer
            .wrap(bytes, position, byteCount)
            .order(java.nio.ByteOrder.LITTLE_ENDIAN)
            .asLongBuffer()
            .get(values)
        position += byteCount
        return values
    }

    fun readULongArray(): ULongArray =
        readLongArray().toULongArray()

    fun readFloatArray(): FloatArray {
        val length = readU32().toInt()
        val byteCount = length * 4
        val values = FloatArray(length)
        java.nio.ByteBuffer
            .wrap(bytes, position, byteCount)
            .order(java.nio.ByteOrder.LITTLE_ENDIAN)
            .asFloatBuffer()
            .get(values)
        position += byteCount
        return values
    }

    fun readDoubleArray(): DoubleArray {
        val length = readU32().toInt()
        val byteCount = length * 8
        val values = DoubleArray(length)
        java.nio.ByteBuffer
            .wrap(bytes, position, byteCount)
            .order(java.nio.ByteOrder.LITTLE_ENDIAN)
            .asDoubleBuffer()
            .get(values)
        position += byteCount
        return values
    }

    fun <T> readOptionalValue(read: (WireReader) -> T): T? = readOptional(read)

    fun <T> readSequence(read: (WireReader) -> T): List<T> {
        val length = readU32().toInt()
        return List(length) { read(this) }
    }

    fun <K, V> readMap(readKey: (WireReader) -> K, readValue: (WireReader) -> V): Map<K, V> {
        val length = readU32().toInt()
        val values = LinkedHashMap<K, V>(length)
        repeat(length) {
            val key = readKey(this)
            if (values.containsKey(key)) {
                throw IllegalArgumentException("duplicate map key")
            }
            values[key] = readValue(this)
        }
        return values
    }

    private inline fun <T> readOptional(read: (WireReader) -> T): T? {
        return when (readU8()) {
            0.toUByte() -> null
            1.toUByte() -> read(this)
            else -> throw IllegalArgumentException("invalid optional wire tag")
        }
    }
}

internal class WireWriter(initialCapacity: Int) {
    private var buffer = java.nio.ByteBuffer
        .allocateDirect(initialCapacity)
        .order(java.nio.ByteOrder.LITTLE_ENDIAN)
    private var position = 0

    fun reset(requiredCapacity: Int) {
        if (buffer.capacity() < requiredCapacity) {
            buffer = java.nio.ByteBuffer
                .allocateDirect(requiredCapacity)
                .order(java.nio.ByteOrder.LITTLE_ENDIAN)
        }
        position = 0
    }

    fun toByteArray(): ByteArray {
        val bytes = ByteArray(position)
        val view = buffer.duplicate()
        view.position(0)
        view.get(bytes, 0, position)
        return bytes
    }

    fun directBuffer(): java.nio.ByteBuffer = buffer

    fun size(): Int = position

    fun writeBool(value: Boolean) {
        ensureCapacity(1)
        buffer.put(position, if (value) 1.toByte() else 0.toByte())
        position += 1
    }

    fun writeI8(value: Byte) {
        ensureCapacity(1)
        buffer.put(position, value)
        position += 1
    }

    fun writeU8(value: UByte) {
        writeI8(value.toByte())
    }

    fun writeI16(value: Short) {
        ensureCapacity(2)
        buffer.putShort(position, value)
        position += 2
    }

    fun writeU16(value: UShort) {
        writeI16(value.toShort())
    }

    fun writeI32(value: Int) {
        ensureCapacity(4)
        buffer.putInt(position, value)
        position += 4
    }

    fun writeU32(value: UInt) {
        writeI32(value.toInt())
    }

    fun writeI64(value: Long) {
        ensureCapacity(8)
        buffer.putLong(position, value)
        position += 8
    }

    fun writeU64(value: ULong) {
        writeI64(value.toLong())
    }

    fun writeF32(value: Float) {
        writeI32(java.lang.Float.floatToRawIntBits(value))
    }

    fun writeF64(value: Double) {
        writeI64(java.lang.Double.doubleToRawLongBits(value))
    }

    fun writeOptionalBool(value: Boolean?) = writeOptional(value) { writer, present ->
        writer.writeBool(present)
    }

    fun writeOptionalI8(value: Byte?) = writeOptional(value) { writer, present ->
        writer.writeI8(present)
    }

    fun writeOptionalU8(value: UByte?) = writeOptional(value) { writer, present ->
        writer.writeU8(present)
    }

    fun writeOptionalI16(value: Short?) = writeOptional(value) { writer, present ->
        writer.writeI16(present)
    }

    fun writeOptionalU16(value: UShort?) = writeOptional(value) { writer, present ->
        writer.writeU16(present)
    }

    fun writeOptionalI32(value: Int?) = writeOptional(value) { writer, present ->
        writer.writeI32(present)
    }

    fun writeOptionalU32(value: UInt?) = writeOptional(value) { writer, present ->
        writer.writeU32(present)
    }

    fun writeOptionalI64(value: Long?) = writeOptional(value) { writer, present ->
        writer.writeI64(present)
    }

    fun writeOptionalU64(value: ULong?) = writeOptional(value) { writer, present ->
        writer.writeU64(present)
    }

    fun writeOptionalF32(value: Float?) = writeOptional(value) { writer, present ->
        writer.writeF32(present)
    }

    fun writeOptionalF64(value: Double?) = writeOptional(value) { writer, present ->
        writer.writeF64(present)
    }

    fun writeString(value: String) {
        val bytes = value.toByteArray(Charsets.UTF_8)
        writeU32(bytes.size.toUInt())
        writeBytesRaw(bytes)
    }

    fun writeBytes(value: ByteArray) {
        writeU32(value.size.toUInt())
        writeBytesRaw(value)
    }

    fun writeBooleanArray(values: BooleanArray) {
        writeU32(values.size.toUInt())
        values.forEach { writeBool(it) }
    }

    fun writeByteArray(values: ByteArray) = writeBytes(values)

    fun writeShortArray(values: ShortArray) {
        writeU32(values.size.toUInt())
        val byteCount = values.size * 2
        ensureCapacity(byteCount)
        val view = buffer.duplicate().order(java.nio.ByteOrder.LITTLE_ENDIAN)
        view.position(position)
        view.asShortBuffer().put(values)
        position += byteCount
    }

    fun writeUShortArray(values: UShortArray) =
        writeShortArray(values.asShortArray())

    fun writeIntArray(values: IntArray) {
        writeU32(values.size.toUInt())
        val byteCount = values.size * 4
        ensureCapacity(byteCount)
        val view = buffer.duplicate().order(java.nio.ByteOrder.LITTLE_ENDIAN)
        view.position(position)
        view.asIntBuffer().put(values)
        position += byteCount
    }

    fun writeUIntArray(values: UIntArray) =
        writeIntArray(values.asIntArray())

    fun writeLongArray(values: LongArray) {
        writeU32(values.size.toUInt())
        val byteCount = values.size * 8
        ensureCapacity(byteCount)
        val view = buffer.duplicate().order(java.nio.ByteOrder.LITTLE_ENDIAN)
        view.position(position)
        view.asLongBuffer().put(values)
        position += byteCount
    }

    fun writeULongArray(values: ULongArray) =
        writeLongArray(values.asLongArray())

    fun writeFloatArray(values: FloatArray) {
        writeU32(values.size.toUInt())
        val byteCount = values.size * 4
        ensureCapacity(byteCount)
        val view = buffer.duplicate().order(java.nio.ByteOrder.LITTLE_ENDIAN)
        view.position(position)
        view.asFloatBuffer().put(values)
        position += byteCount
    }

    fun writeDoubleArray(values: DoubleArray) {
        writeU32(values.size.toUInt())
        val byteCount = values.size * 8
        ensureCapacity(byteCount)
        val view = buffer.duplicate().order(java.nio.ByteOrder.LITTLE_ENDIAN)
        view.position(position)
        view.asDoubleBuffer().put(values)
        position += byteCount
    }

    fun <T> writeOptionalValue(value: T?, write: (WireWriter, T) -> Unit) {
        writeOptional(value, write)
    }

    fun <T> writeSequence(value: Iterable<T>, count: Int, write: (WireWriter, T) -> Unit) {
        writeU32(count.toUInt())
        value.forEach { item -> write(this, item) }
    }

    fun <K, V> writeMap(
        value: Map<K, V>,
        writeKey: (WireWriter, K) -> Unit,
        writeValue: (WireWriter, V) -> Unit,
    ) {
        writeU32(value.size.toUInt())
        value.entries.forEach { entry ->
            writeKey(this, entry.key)
            writeValue(this, entry.value)
        }
    }

    private fun writeBytesRaw(bytes: ByteArray) {
        ensureCapacity(bytes.size)
        val view = buffer.duplicate().order(java.nio.ByteOrder.LITTLE_ENDIAN)
        view.position(position)
        view.put(bytes)
        position += bytes.size
    }

    private fun ensureCapacity(needed: Int) {
        val required = position + needed
        if (required <= buffer.capacity()) {
            return
        }
        val nextCapacity = maxOf(buffer.capacity() * 2, required)
        val next = java.nio.ByteBuffer
            .allocateDirect(nextCapacity)
            .order(java.nio.ByteOrder.LITTLE_ENDIAN)
        val source = buffer.duplicate().order(java.nio.ByteOrder.LITTLE_ENDIAN)
        source.limit(position)
        source.position(0)
        next.put(source)
        buffer = next
    }

    private inline fun <T> writeOptional(value: T?, write: (WireWriter, T) -> Unit) {
        if (value == null) {
            writeU8(0.toUByte())
            return
        }
        writeU8(1.toUByte())
        write(this, value)
    }
}

private const val MAX_CACHED_WIRE_WRITER_BYTES: Int = 1024 * 1024

private class WireWriterPoolState(private val cacheSize: Int = 4) {
    private val cachedWriters: Array<WireWriter?> = arrayOfNulls(cacheSize)
    private var depth = 0

    fun acquire(requiredCapacity: Int): BorrowedWireWriter {
        val slot = depth
        depth = slot + 1
        val shouldCache = requiredCapacity <= MAX_CACHED_WIRE_WRITER_BYTES && slot < cacheSize
        val writer = if (shouldCache) {
            cachedWriters[slot] ?: WireWriter(requiredCapacity).also { cachedWriters[slot] = it }
        } else {
            WireWriter(requiredCapacity)
        }

        writer.reset(requiredCapacity)
        return BorrowedWireWriter(this, writer)
    }

    fun release() {
        depth -= 1
    }
}

private class BorrowedWireWriter(
    private val state: WireWriterPoolState,
    val writer: WireWriter,
) : AutoCloseable {
    fun bytes(): ByteArray = writer.toByteArray()

    fun directBuffer(): java.nio.ByteBuffer = writer.directBuffer()

    fun size(): Int = writer.size()

    override fun close() {
        state.release()
    }
}

private object WireWriterPool {
    private val state: ThreadLocal<WireWriterPoolState> =
        ThreadLocal.withInitial { WireWriterPoolState() }

    fun acquire(requiredCapacity: Int): BorrowedWireWriter {
        val poolState = state.get() ?: WireWriterPoolState().also { state.set(it) }
        return poolState.acquire(requiredCapacity)
    }
}

private inline fun <K, V> Map<K, V>.wireSize(
    keySize: (K) -> Int,
    valueSize: (V) -> Int,
): Int = 4 + entries.sumOf { entry -> keySize(entry.key) + valueSize(entry.value) }

@Suppress("FunctionName")
private object Native {

    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_class_access_flags(c: Int, flags: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_superclass(c: Int, superclass: java.nio.ByteBuffer, __boltffi_superclass_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_interface(c: Int, interface_descriptor: java.nio.ByteBuffer, __boltffi_interface_descriptor_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_remove_class(c: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_create_class(dex_index: Int, descriptor: java.nio.ByteBuffer, __boltffi_descriptor_len: Int, flags: Int, superclass: java.nio.ByteBuffer, __boltffi_superclass_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_definal_class(c: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_superclass_chain(c: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_method(c: Int, method: java.nio.ByteBuffer, __boltffi_method_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_remove_method(m: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_method_access_flags(m: Int, flags: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_clone_method(m: Int, new_name: java.nio.ByteBuffer, __boltffi_new_name_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_field(c: Int, `field`: java.nio.ByteBuffer, __boltffi_field_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_remove_field(c: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_field_access_flags(c: Int, field_name: java.nio.ByteBuffer, __boltffi_field_name_len: Int, flags: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_static_field_value(c: Int, field_name: java.nio.ByteBuffer, __boltffi_field_name_len: Int, value: java.nio.ByteBuffer, __boltffi_value_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_class_annotation(c: Int, `annotation`: java.nio.ByteBuffer, __boltffi_annotation_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_method_annotation(m: Int, `annotation`: java.nio.ByteBuffer, __boltffi_annotation_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_field_annotation(c: Int, field_name: java.nio.ByteBuffer, __boltffi_field_name_len: Int, `annotation`: java.nio.ByteBuffer, __boltffi_annotation_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_dex_count(): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_method_dex(m: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_string(d: Int, s: java.nio.ByteBuffer, __boltffi_s_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_type(d: Int, descriptor: java.nio.ByteBuffer, __boltffi_descriptor_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_proto(d: Int, proto: java.nio.ByteBuffer, __boltffi_proto_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_method(d: Int, descriptor: java.nio.ByteBuffer, __boltffi_descriptor_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int, proto: java.nio.ByteBuffer, __boltffi_proto_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_field(d: Int, descriptor: java.nio.ByteBuffer, __boltffi_descriptor_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int, field_type: java.nio.ByteBuffer, __boltffi_field_type_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_find_string_idx(d: Int, s: java.nio.ByteBuffer, __boltffi_s_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_get_string(d: Int, idx: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_get_type_descriptor(d: Int, idx: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_build_lookups(d: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_method(class_descriptor: java.nio.ByteBuffer, __boltffi_class_descriptor_len: Int, method_name: java.nio.ByteBuffer, __boltffi_method_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_method_by_name(name: java.nio.ByteBuffer, __boltffi_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_name(name: java.nio.ByteBuffer, __boltffi_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_strings(strings: java.nio.ByteBuffer, __boltffi_strings_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_proto(return_type: java.nio.ByteBuffer, __boltffi_return_type_len: Int, parameter_types: java.nio.ByteBuffer, __boltffi_parameter_types_len: Int, parameter: java.nio.ByteBuffer, __boltffi_parameter_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_opcodes(pattern: IntArray): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_method_by_fingerprint(fp: java.nio.ByteBuffer, __boltffi_fp_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_fingerprint(fp: java.nio.ByteBuffer, __boltffi_fp_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_class(descriptor: java.nio.ByteBuffer, __boltffi_descriptor_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_get_all_classes(): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_get_method_info(m: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_get_class_info(c: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_class_direct_methods(c: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_class_virtual_methods(c: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_lookup_class_fields(c: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_set_instructions(m: Int, insns: java.nio.ByteBuffer, __boltffi_insns_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_body(m: Int, registers_size: Short, outs_size: Short, insns: java.nio.ByteBuffer, __boltffi_insns_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_insert_instructions(m: Int, index: Int, insns: java.nio.ByteBuffer, __boltffi_insns_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_insert_before_instruction(m: Int, index: Int, insns: java.nio.ByteBuffer, __boltffi_insns_len: Int): Boolean
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_instruction(m: Int, index: Int, insn: java.nio.ByteBuffer, __boltffi_insn_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_remove_instructions(m: Int, index: Int, count: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_return_early(m: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_return_early_int(m: Int, value: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_return_early_object_null(m: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_return_early_wide(m: Int, value: Long): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_strings(m: Int, old: java.nio.ByteBuffer, __boltffi_old_len: Int, new: java.nio.ByteBuffer, __boltffi_new_len: Int, all: Boolean): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_literals(m: Int, old: Long, new: Long, all: Boolean): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_method_call(m: Int, index: Int, new_class: java.nio.ByteBuffer, __boltffi_new_class_len: Int, new_name: java.nio.ByteBuffer, __boltffi_new_name_len: Int, new_proto: java.nio.ByteBuffer, __boltffi_new_proto_len: Int): Boolean
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_redirect_method_calls(from: java.nio.ByteBuffer, __boltffi_from_len: Int, to: java.nio.ByteBuffer, __boltffi_to_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_insert_invoke_static(m: Int, index: Int, class_name: java.nio.ByteBuffer, __boltffi_class_name_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int, proto: java.nio.ByteBuffer, __boltffi_proto_len: Int, registers: ShortArray): Boolean
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_mutation_insert_invoke_static_with_move_result(m: Int, index: Int, class_name: java.nio.ByteBuffer, __boltffi_class_name_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int, proto: java.nio.ByteBuffer, __boltffi_proto_len: Int, registers: ShortArray, result_register: Short, is_object: Boolean): Boolean
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_ensure_outs_size(m: Int, min_outs_size: Short): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_grow_local_registers(m: Int, additional_locals: Short): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_registers_size(m: Int): Short
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_ins_size(m: Int): Short
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_outs_size(m: Int): Short
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_find_free_register(m: Int, at_index: Int, exclude: ShortArray): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_find_free_registers(m: Int, at_index: Int, count: Int, exclude: ShortArray): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_find_contiguous_free_registers(m: Int, at_index: Int, count: Int, exclude: ShortArray): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_instruction_register(m: Int, index: Int, position: Int): Short
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_registers_instruction_wide_literal(m: Int, index: Int): Long
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_get_instructions(m: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_get_instruction(m: Int, index: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_count(m: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first(m: Int, start: Int, op: Short): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_reversed(m: Int, start: Int, op: Short): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_literal(m: Int, literal: Long): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_literal_reversed(m: Int, literal: Long): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_string(m: Int, s: java.nio.ByteBuffer, __boltffi_s_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_find_all_indices(m: Int, op: Short): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_method_call(m: Int, defining_class: java.nio.ByteBuffer, __boltffi_defining_class_len: Int, method_name: java.nio.ByteBuffer, __boltffi_method_name_len: Int, start: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_field_access(m: Int, op: Int, field_type: java.nio.ByteBuffer, __boltffi_field_type_len: Int, defining_class: java.nio.ByteBuffer, __boltffi_defining_class_len: Int, start: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_opcode_sequence(m: Int, opcodes: IntArray, start: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_literal(literal: Long): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_string(s: java.nio.ByteBuffer, __boltffi_s_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_string_contains(substring: java.nio.ByteBuffer, __boltffi_substring_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_resource_id(res_type: java.nio.ByteBuffer, __boltffi_res_type_len: Int, res_name: java.nio.ByteBuffer, __boltffi_res_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_find_method_call_sites(class_names: java.nio.ByteBuffer, __boltffi_class_names_len: Int, method_names: java.nio.ByteBuffer, __boltffi_method_names_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_find_field_access_sites(class_names: java.nio.ByteBuffer, __boltffi_class_names_len: Int, field_names: java.nio.ByteBuffer, __boltffi_field_names_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_invoke(defining_class: java.nio.ByteBuffer, __boltffi_defining_class_len: Int, method_name: java.nio.ByteBuffer, __boltffi_method_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_all_method_handles(): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_string_ref(m: Int, index: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_method_ref(m: Int, index: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_field_ref(m: Int, index: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_type_ref(m: Int, index: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_files_component_names(): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_files_file_list(component: java.nio.ByteBuffer, __boltffi_component_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_files_file_read(component: java.nio.ByteBuffer, __boltffi_component_len: Int, apk_path: java.nio.ByteBuffer, __boltffi_apk_path_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_files_file_source(component: java.nio.ByteBuffer, __boltffi_component_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_files_file_signers(component: java.nio.ByteBuffer, __boltffi_component_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_files_file_inject(component: java.nio.ByteBuffer, __boltffi_component_len: Int, apk_path: java.nio.ByteBuffer, __boltffi_apk_path_len: Int, `data`: java.nio.ByteBuffer, __boltffi_data_len: Int, stored: Boolean): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_files_file_delete(component: java.nio.ByteBuffer, __boltffi_component_len: Int, apk_path: java.nio.ByteBuffer, __boltffi_apk_path_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_files_file_copy(component: java.nio.ByteBuffer, __boltffi_component_len: Int, bundle_relative: java.nio.ByteBuffer, __boltffi_bundle_relative_len: Int, apk_path: java.nio.ByteBuffer, __boltffi_apk_path_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_log_host_log_info(msg: java.nio.ByteBuffer, __boltffi_msg_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_log_host_log_warn(msg: java.nio.ByteBuffer, __boltffi_msg_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_log_host_log_debug(msg: java.nio.ByteBuffer, __boltffi_msg_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_package_name(component: java.nio.ByteBuffer, __boltffi_component_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_version_code(component: java.nio.ByteBuffer, __boltffi_component_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_version_name(component: java.nio.ByteBuffer, __boltffi_component_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_min_sdk_version(component: java.nio.ByteBuffer, __boltffi_component_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_split_name(component: java.nio.ByteBuffer, __boltffi_component_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_version_code(component: java.nio.ByteBuffer, __boltffi_component_len: Int, code: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_version_name(component: java.nio.ByteBuffer, __boltffi_component_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_min_sdk(component: java.nio.ByteBuffer, __boltffi_component_len: Int, sdk: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_add_permission(component: java.nio.ByteBuffer, __boltffi_component_len: Int, permission: java.nio.ByteBuffer, __boltffi_permission_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_attribute_int(component: java.nio.ByteBuffer, __boltffi_component_len: Int, element_name: java.nio.ByteBuffer, __boltffi_element_name_len: Int, attr_name: java.nio.ByteBuffer, __boltffi_attr_name_len: Int, value: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_attribute_string(component: java.nio.ByteBuffer, __boltffi_component_len: Int, element_name: java.nio.ByteBuffer, __boltffi_element_name_len: Int, attr_name: java.nio.ByteBuffer, __boltffi_attr_name_len: Int, value: java.nio.ByteBuffer, __boltffi_value_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_activity_config_changes(component: java.nio.ByteBuffer, __boltffi_component_len: Int, activity_name: java.nio.ByteBuffer, __boltffi_activity_name_len: Int, config_changes: java.nio.ByteBuffer, __boltffi_config_changes_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_add_intent_filter(component: java.nio.ByteBuffer, __boltffi_component_len: Int, activity_name: java.nio.ByteBuffer, __boltffi_activity_name_len: Int, action: java.nio.ByteBuffer, __boltffi_action_len: Int, category: java.nio.ByteBuffer, __boltffi_category_len: Int, mime_type: java.nio.ByteBuffer, __boltffi_mime_type_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_add_activity_alias(component: java.nio.ByteBuffer, __boltffi_component_len: Int, target_activity: java.nio.ByteBuffer, __boltffi_target_activity_len: Int, alias_name: java.nio.ByteBuffer, __boltffi_alias_name_len: Int, enabled: Boolean, label: java.nio.ByteBuffer, __boltffi_label_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_copy_intent_filters(component: java.nio.ByteBuffer, __boltffi_component_len: Int, from_activity: java.nio.ByteBuffer, __boltffi_from_activity_len: Int, to_activity: java.nio.ByteBuffer, __boltffi_to_activity_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_manifest_manifest_get_document(component: java.nio.ByteBuffer, __boltffi_component_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_options_option_get_string(key: java.nio.ByteBuffer, __boltffi_key_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_options_option_get_bool(key: java.nio.ByteBuffer, __boltffi_key_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_options_option_get_int(key: java.nio.ByteBuffer, __boltffi_key_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_options_option_get_float(key: java.nio.ByteBuffer, __boltffi_key_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_options_option_get_string_list(key: java.nio.ByteBuffer, __boltffi_key_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_options_option_get_path(key: java.nio.ByteBuffer, __boltffi_key_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_options_option_list_path_contents(key: java.nio.ByteBuffer, __boltffi_key_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_options_option_read_path_file(key: java.nio.ByteBuffer, __boltffi_key_len: Int, relative_path: java.nio.ByteBuffer, __boltffi_relative_path_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_component_names(): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_component_for(res_type: java.nio.ByteBuffer, __boltffi_res_type_len: Int, res_name: java.nio.ByteBuffer, __boltffi_res_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_component_for_id(res_id: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_id(component: java.nio.ByteBuffer, __boltffi_component_len: Int, res_type: java.nio.ByteBuffer, __boltffi_res_type_len: Int, res_name: java.nio.ByteBuffer, __boltffi_res_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_exists(component: java.nio.ByteBuffer, __boltffi_component_len: Int, res_type: java.nio.ByteBuffer, __boltffi_res_type_len: Int, res_name: java.nio.ByteBuffer, __boltffi_res_name_len: Int): Boolean
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_get_string(component: java.nio.ByteBuffer, __boltffi_component_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_set_string(component: java.nio.ByteBuffer, __boltffi_component_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int, value: java.nio.ByteBuffer, __boltffi_value_len: Int): Boolean
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_add(component: java.nio.ByteBuffer, __boltffi_component_len: Int, res_type: java.nio.ByteBuffer, __boltffi_res_type_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int, value: java.nio.ByteBuffer, __boltffi_value_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_add_id(component: java.nio.ByteBuffer, __boltffi_component_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_add_raw(component: java.nio.ByteBuffer, __boltffi_component_len: Int, res_type: java.nio.ByteBuffer, __boltffi_res_type_len: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int, data_type: Byte, `data`: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_get_raw(component: java.nio.ByteBuffer, __boltffi_component_len: Int, res_type: java.nio.ByteBuffer, __boltffi_res_type_len: Int, res_name: java.nio.ByteBuffer, __boltffi_res_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_copy(bundle_relative: java.nio.ByteBuffer, __boltffi_bundle_relative_len: Int, apk_path: java.nio.ByteBuffer, __boltffi_apk_path_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_copy_group(res_type: java.nio.ByteBuffer, __boltffi_res_type_len: Int, files: java.nio.ByteBuffer, __boltffi_files_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_inject(apk_path: java.nio.ByteBuffer, __boltffi_apk_path_len: Int, `data`: java.nio.ByteBuffer, __boltffi_data_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_delete(apk_path: java.nio.ByteBuffer, __boltffi_apk_path_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_list(prefix: java.nio.ByteBuffer, __boltffi_prefix_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_pool_get(component: java.nio.ByteBuffer, __boltffi_component_len: Int, index: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_pool_set(component: java.nio.ByteBuffer, __boltffi_component_len: Int, index: Int, value: java.nio.ByteBuffer, __boltffi_value_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_pool_add(component: java.nio.ByteBuffer, __boltffi_component_len: Int, value: java.nio.ByteBuffer, __boltffi_value_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_pool_find_refs(component: java.nio.ByteBuffer, __boltffi_component_len: Int, string_index: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_resources_res_replace_entry(component: java.nio.ByteBuffer, __boltffi_component_len: Int, res_id: Int, new_string_index: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_open(component: java.nio.ByteBuffer, __boltffi_component_len: Int, apk_path: java.nio.ByteBuffer, __boltffi_apk_path_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_close(doc: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_root(doc: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_find_by_tag(doc: Int, tag: java.nio.ByteBuffer, __boltffi_tag_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_find_by_attribute(doc: Int, attr_name: java.nio.ByteBuffer, __boltffi_attr_name_len: Int, attr_value: java.nio.ByteBuffer, __boltffi_attr_value_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_children(doc: Int, el: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_parent(doc: Int, el: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_tag_name(doc: Int, el: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_get_attribute(doc: Int, el: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int): ByteArray?
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_set_attribute(doc: Int, el: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int, value: java.nio.ByteBuffer, __boltffi_value_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_set_attribute_ref(doc: Int, el: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int, res_id: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_remove_attribute(doc: Int, el: Int, name: java.nio.ByteBuffer, __boltffi_name_len: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_create_element(doc: Int, tag: java.nio.ByteBuffer, __boltffi_tag_len: Int): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_append_child(doc: Int, parent: Int, child: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_insert_before(doc: Int, child: Int, before: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_remove_element(doc: Int, el: Int): Unit
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_xml_xml_clone_element(doc: Int, el: Int, deep: Boolean): Int
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_ctx_is_active(): Boolean
    @JvmStatic external fun boltffi_function_reseam_patcher_kotlin_version(): ByteArray?
}


data class MethodInfo(
    val classDescriptor: String,
    val methodName: String,
    val proto: String,
    val accessFlags: UInt,
    val dexIndex: UInt,
    val registerCount: UShort,
    val insSize: UShort,
    val outsSize: UShort,
    val instructionCount: UInt
) {
    internal fun wireSize(): Int {
        return 4 + Utf8Codec.maxBytes(this.classDescriptor) + 4 + Utf8Codec.maxBytes(this.methodName) + 4 + Utf8Codec.maxBytes(this.proto) + 4 + 4 + 2 + 2 + 2 + 4
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeString(this.classDescriptor)
        writer.writeString(this.methodName)
        writer.writeString(this.proto)
        writer.writeU32(this.accessFlags)
        writer.writeU32(this.dexIndex)
        writer.writeU16(this.registerCount)
        writer.writeU16(this.insSize)
        writer.writeU16(this.outsSize)
        writer.writeU32(this.instructionCount)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): MethodInfo {
            return MethodInfo(
                reader.readString(),
                reader.readString(),
                reader.readString(),
                reader.readU32(),
                reader.readU32(),
                reader.readU16(),
                reader.readU16(),
                reader.readU16(),
                reader.readU32()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): MethodInfo {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class ClassInfo(
    val descriptor: String,
    val accessFlags: UInt,
    val superclass: String?,
    val interfaces: List<String>,
    val sourceFile: String?,
    val dexIndex: UInt,
    val directMethodCount: UInt,
    val virtualMethodCount: UInt,
    val staticFieldCount: UInt,
    val instanceFieldCount: UInt
) {
    internal fun wireSize(): Int {
        return 4 + Utf8Codec.maxBytes(this.descriptor) + 4 + 1 + (this.superclass?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0) + 4 + this.interfaces.sumOf { __boltffi_value_0 -> (4 + Utf8Codec.maxBytes(__boltffi_value_0)).toInt() } + 1 + (this.sourceFile?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0) + 4 + 4 + 4 + 4 + 4
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeString(this.descriptor)
        writer.writeU32(this.accessFlags)
        writer.writeOptionalValue(this.superclass, { writer, __boltffi_value_0 -> writer.writeString(__boltffi_value_0) })
        writer.writeSequence(this.interfaces, this.interfaces.size, { writer, __boltffi_value_0 -> writer.writeString(__boltffi_value_0) })
        writer.writeOptionalValue(this.sourceFile, { writer, __boltffi_value_0 -> writer.writeString(__boltffi_value_0) })
        writer.writeU32(this.dexIndex)
        writer.writeU32(this.directMethodCount)
        writer.writeU32(this.virtualMethodCount)
        writer.writeU32(this.staticFieldCount)
        writer.writeU32(this.instanceFieldCount)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): ClassInfo {
            return ClassInfo(
                reader.readString(),
                reader.readU32(),
                reader.readOptionalValue({ reader -> reader.readString() }),
                reader.readSequence({ reader -> reader.readString() }),
                reader.readOptionalValue({ reader -> reader.readString() }),
                reader.readU32(),
                reader.readU32(),
                reader.readU32(),
                reader.readU32(),
                reader.readU32()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): ClassInfo {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class FieldInfo(
    val classDescriptor: String,
    val name: String,
    val fieldType: String,
    val accessFlags: UInt,
    val initialValue: EncodedVal?
) {
    internal fun wireSize(): Int {
        return 4 + Utf8Codec.maxBytes(this.classDescriptor) + 4 + Utf8Codec.maxBytes(this.name) + 4 + Utf8Codec.maxBytes(this.fieldType) + 4 + 1 + (this.initialValue?.let { __boltffi_value_0 -> __boltffi_value_0.wireSize() } ?: 0)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeString(this.classDescriptor)
        writer.writeString(this.name)
        writer.writeString(this.fieldType)
        writer.writeU32(this.accessFlags)
        writer.writeOptionalValue(this.initialValue, { writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(writer) })
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): FieldInfo {
            return FieldInfo(
                reader.readString(),
                reader.readString(),
                reader.readString(),
                reader.readU32(),
                reader.readOptionalValue({ reader -> EncodedVal.fromReader(reader) })
            )
        }

        internal fun fromByteArray(bytes: ByteArray): FieldInfo {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class FingerprintDef(
    val name: String?,
    val definingClass: String?,
    val accessFlags: UInt?,
    val returnType: String?,
    val parameters: List<String>?,
    val opcodes: IntArray?,
    val strings: List<String>?,
    val literals: LongArray?
) {
    internal fun wireSize(): Int {
        return 1 + (this.name?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0) + 1 + (this.definingClass?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0) + 1 + (this.accessFlags?.let { __boltffi_value_0 -> 4 } ?: 0) + 1 + (this.returnType?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0) + 1 + (this.parameters?.let { __boltffi_value_0 -> 4 + __boltffi_value_0.sumOf { __boltffi_value_1 -> (4 + Utf8Codec.maxBytes(__boltffi_value_1)).toInt() } } ?: 0) + 1 + (this.opcodes?.let { __boltffi_value_0 -> 4 + __boltffi_value_0.size * 4 } ?: 0) + 1 + (this.strings?.let { __boltffi_value_0 -> 4 + __boltffi_value_0.sumOf { __boltffi_value_1 -> (4 + Utf8Codec.maxBytes(__boltffi_value_1)).toInt() } } ?: 0) + 1 + (this.literals?.let { __boltffi_value_0 -> 4 + __boltffi_value_0.size * 8 } ?: 0)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeOptionalValue(this.name, { writer, __boltffi_value_0 -> writer.writeString(__boltffi_value_0) })
        writer.writeOptionalValue(this.definingClass, { writer, __boltffi_value_0 -> writer.writeString(__boltffi_value_0) })
        writer.writeOptionalValue(this.accessFlags, { writer, __boltffi_value_0 -> writer.writeU32(__boltffi_value_0) })
        writer.writeOptionalValue(this.returnType, { writer, __boltffi_value_0 -> writer.writeString(__boltffi_value_0) })
        writer.writeOptionalValue(this.parameters, { writer, __boltffi_value_0 -> writer.writeSequence(__boltffi_value_0, __boltffi_value_0.size, { writer, __boltffi_value_1 -> writer.writeString(__boltffi_value_1) }) })
        writer.writeOptionalValue(this.opcodes, { writer, __boltffi_value_0 -> writer.writeIntArray(__boltffi_value_0) })
        writer.writeOptionalValue(this.strings, { writer, __boltffi_value_0 -> writer.writeSequence(__boltffi_value_0, __boltffi_value_0.size, { writer, __boltffi_value_1 -> writer.writeString(__boltffi_value_1) }) })
        writer.writeOptionalValue(this.literals, { writer, __boltffi_value_0 -> writer.writeLongArray(__boltffi_value_0) })
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): FingerprintDef {
            return FingerprintDef(
                reader.readOptionalValue({ reader -> reader.readString() }),
                reader.readOptionalValue({ reader -> reader.readString() }),
                reader.readOptionalValue({ reader -> reader.readU32() }),
                reader.readOptionalValue({ reader -> reader.readString() }),
                reader.readOptionalValue({ reader -> reader.readSequence({ reader -> reader.readString() }) }),
                reader.readOptionalValue({ reader -> reader.readIntArray() }),
                reader.readOptionalValue({ reader -> reader.readSequence({ reader -> reader.readString() }) }),
                reader.readOptionalValue({ reader -> reader.readLongArray() })
            )
        }

        internal fun fromByteArray(bytes: ByteArray): FingerprintDef {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class FingerprintResult(
    val method: UInt,
    val matchedCount: UInt
) {
    internal fun wireSize(): Int {
        return 8
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU32(method)
        writer.writeU32(matchedCount)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putInt(offset, method.toInt())
        buffer.putInt(offset + 4, matchedCount.toInt())
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 8
        internal fun fromReader(reader: WireReader): FingerprintResult {
            return FingerprintResult(
                reader.readU32(),
                reader.readU32()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): FingerprintResult {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): FingerprintResult {
            return FingerprintResult(
                buffer.getInt(offset).toUInt(),
                buffer.getInt(offset + 4).toUInt()
            )
        }
    }
}


data class InstructionHit(
    val method: UInt,
    val index: UInt
) {
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putInt(offset, method.toInt())
        buffer.putInt(offset + 4, index.toInt())
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 8

        internal fun fromByteArray(bytes: ByteArray): InstructionHit {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): InstructionHit {
            return InstructionHit(
                buffer.getInt(offset).toUInt(),
                buffer.getInt(offset + 4).toUInt()
            )
        }
    }
}


data class MethodCallSiteResult(
    val method: UInt,
    val index: UInt,
    val targetIndex: UInt
) {
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putInt(offset, method.toInt())
        buffer.putInt(offset + 4, index.toInt())
        buffer.putInt(offset + 8, targetIndex.toInt())
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 12

        internal fun fromByteArray(bytes: ByteArray): MethodCallSiteResult {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): MethodCallSiteResult {
            return MethodCallSiteResult(
                buffer.getInt(offset).toUInt(),
                buffer.getInt(offset + 4).toUInt(),
                buffer.getInt(offset + 8).toUInt()
            )
        }
    }
}


data class NewMethod(
    val name: String,
    val proto: String,
    val accessFlags: UInt,
    val registersSize: UShort,
    val insSize: UShort,
    val outsSize: UShort,
    val instructions: List<Instruction>,
    val tries: List<TryItem>,
    val catchHandlers: List<CatchHandler>
) {
    internal fun wireSize(): Int {
        return 4 + Utf8Codec.maxBytes(this.name) + 4 + Utf8Codec.maxBytes(this.proto) + 4 + 2 + 2 + 2 + 4 + this.instructions.sumOf { __boltffi_value_0 -> (__boltffi_value_0.wireSize()).toInt() } + 4 + this.tries.sumOf { __boltffi_value_0 -> (__boltffi_value_0.wireSize()).toInt() } + 4 + this.catchHandlers.sumOf { __boltffi_value_0 -> (__boltffi_value_0.wireSize()).toInt() }
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeString(this.name)
        writer.writeString(this.proto)
        writer.writeU32(this.accessFlags)
        writer.writeU16(this.registersSize)
        writer.writeU16(this.insSize)
        writer.writeU16(this.outsSize)
        writer.writeSequence(this.instructions, this.instructions.size, { writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(writer) })
        writer.writeSequence(this.tries, this.tries.size, { writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(writer) })
        writer.writeSequence(this.catchHandlers, this.catchHandlers.size, { writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(writer) })
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): NewMethod {
            return NewMethod(
                reader.readString(),
                reader.readString(),
                reader.readU32(),
                reader.readU16(),
                reader.readU16(),
                reader.readU16(),
                reader.readSequence({ reader -> Instruction.fromReader(reader) }),
                reader.readSequence({ reader -> TryItem.fromReader(reader) }),
                reader.readSequence({ reader -> CatchHandler.fromReader(reader) })
            )
        }

        internal fun fromByteArray(bytes: ByteArray): NewMethod {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class TryItem(
    val startAddr: UInt,
    val insnCount: UShort,
    val handlerIdx: UInt
) {
    internal fun wireSize(): Int {
        return 10
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU32(startAddr)
        writer.writeU16(insnCount)
        writer.writeU32(handlerIdx)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putInt(offset, startAddr.toInt())
        buffer.putShort(offset + 4, insnCount.toShort())
        buffer.putInt(offset + 8, handlerIdx.toInt())
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 12
        internal fun fromReader(reader: WireReader): TryItem {
            return TryItem(
                reader.readU32(),
                reader.readU16(),
                reader.readU32()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): TryItem {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): TryItem {
            return TryItem(
                buffer.getInt(offset).toUInt(),
                buffer.getShort(offset + 4).toUShort(),
                buffer.getInt(offset + 8).toUInt()
            )
        }
    }
}


data class CatchHandler(
    val typedCatches: List<TypedCatch>,
    val catchAllAddr: UInt?
) {
    internal fun wireSize(): Int {
        return 4 + this.typedCatches.sumOf { __boltffi_value_0 -> (__boltffi_value_0.wireSize()).toInt() } + 1 + (this.catchAllAddr?.let { __boltffi_value_0 -> 4 } ?: 0)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeSequence(this.typedCatches, this.typedCatches.size, { writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(writer) })
        writer.writeOptionalValue(this.catchAllAddr, { writer, __boltffi_value_0 -> writer.writeU32(__boltffi_value_0) })
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): CatchHandler {
            return CatchHandler(
                reader.readSequence({ reader -> TypedCatch.fromReader(reader) }),
                reader.readOptionalValue({ reader -> reader.readU32() })
            )
        }

        internal fun fromByteArray(bytes: ByteArray): CatchHandler {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class TypedCatch(
    val exceptionType: String,
    val addr: UInt
) {
    internal fun wireSize(): Int {
        return 4 + Utf8Codec.maxBytes(this.exceptionType) + 4
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeString(this.exceptionType)
        writer.writeU32(this.addr)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): TypedCatch {
            return TypedCatch(
                reader.readString(),
                reader.readU32()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): TypedCatch {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class NewField(
    val name: String,
    val fieldType: String,
    val accessFlags: UInt,
    val initialValue: EncodedVal?
) {
    internal fun wireSize(): Int {
        return 4 + Utf8Codec.maxBytes(this.name) + 4 + Utf8Codec.maxBytes(this.fieldType) + 4 + 1 + (this.initialValue?.let { __boltffi_value_0 -> __boltffi_value_0.wireSize() } ?: 0)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeString(this.name)
        writer.writeString(this.fieldType)
        writer.writeU32(this.accessFlags)
        writer.writeOptionalValue(this.initialValue, { writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(writer) })
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): NewField {
            return NewField(
                reader.readString(),
                reader.readString(),
                reader.readU32(),
                reader.readOptionalValue({ reader -> EncodedVal.fromReader(reader) })
            )
        }

        internal fun fromByteArray(bytes: ByteArray): NewField {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class AnnotationItem(
    val visibility: UByte,
    val annotationType: String,
    val elements: List<AnnotationElement>
) {
    internal fun wireSize(): Int {
        return 1 + 4 + Utf8Codec.maxBytes(this.annotationType) + 4 + this.elements.sumOf { __boltffi_value_0 -> (__boltffi_value_0.wireSize()).toInt() }
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU8(this.visibility)
        writer.writeString(this.annotationType)
        writer.writeSequence(this.elements, this.elements.size, { writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(writer) })
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): AnnotationItem {
            return AnnotationItem(
                reader.readU8(),
                reader.readString(),
                reader.readSequence({ reader -> AnnotationElement.fromReader(reader) })
            )
        }

        internal fun fromByteArray(bytes: ByteArray): AnnotationItem {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class AnnotationElement(
    val name: String,
    val value: EncodedVal
) {
    internal fun wireSize(): Int {
        return 4 + Utf8Codec.maxBytes(this.name) + this.value.wireSize()
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeString(this.name)
        this.value.writeTo(writer)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): AnnotationElement {
            return AnnotationElement(
                reader.readString(),
                EncodedVal.fromReader(reader)
            )
        }

        internal fun fromByteArray(bytes: ByteArray): AnnotationElement {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class ResourceRef(
    val resId: UInt,
    val keyName: String
) {
    internal fun wireSize(): Int {
        return 4 + 4 + Utf8Codec.maxBytes(this.keyName)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU32(this.resId)
        writer.writeString(this.keyName)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): ResourceRef {
            return ResourceRef(
                reader.readU32(),
                reader.readString()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): ResourceRef {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class MethodRef(
    val definingClass: String,
    val name: String,
    val proto: String
) {
    internal fun wireSize(): Int {
        return 4 + Utf8Codec.maxBytes(this.definingClass) + 4 + Utf8Codec.maxBytes(this.name) + 4 + Utf8Codec.maxBytes(this.proto)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeString(this.definingClass)
        writer.writeString(this.name)
        writer.writeString(this.proto)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): MethodRef {
            return MethodRef(
                reader.readString(),
                reader.readString(),
                reader.readString()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): MethodRef {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class FieldRef(
    val definingClass: String,
    val name: String,
    val fieldType: String
) {
    internal fun wireSize(): Int {
        return 4 + Utf8Codec.maxBytes(this.definingClass) + 4 + Utf8Codec.maxBytes(this.name) + 4 + Utf8Codec.maxBytes(this.fieldType)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeString(this.definingClass)
        writer.writeString(this.name)
        writer.writeString(this.fieldType)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): FieldRef {
            return FieldRef(
                reader.readString(),
                reader.readString(),
                reader.readString()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): FieldRef {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class SimpleInsn(
    val opcode: UShort
) {
    internal fun wireSize(): Int {
        return 2
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(opcode)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putShort(offset, opcode.toShort())
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 2
        internal fun fromReader(reader: WireReader): SimpleInsn {
            return SimpleInsn(
                reader.readU16()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): SimpleInsn {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): SimpleInsn {
            return SimpleInsn(
                buffer.getShort(offset).toUShort()
            )
        }
    }
}


data class Reg1Insn(
    val opcode: UShort,
    val regA: UShort
) {
    internal fun wireSize(): Int {
        return 4
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(opcode)
        writer.writeU16(regA)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putShort(offset, opcode.toShort())
        buffer.putShort(offset + 2, regA.toShort())
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 4
        internal fun fromReader(reader: WireReader): Reg1Insn {
            return Reg1Insn(
                reader.readU16(),
                reader.readU16()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): Reg1Insn {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): Reg1Insn {
            return Reg1Insn(
                buffer.getShort(offset).toUShort(),
                buffer.getShort(offset + 2).toUShort()
            )
        }
    }
}


data class Reg2Insn(
    val opcode: UShort,
    val regA: UShort,
    val regB: UShort
) {
    internal fun wireSize(): Int {
        return 6
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(opcode)
        writer.writeU16(regA)
        writer.writeU16(regB)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putShort(offset, opcode.toShort())
        buffer.putShort(offset + 2, regA.toShort())
        buffer.putShort(offset + 4, regB.toShort())
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 6
        internal fun fromReader(reader: WireReader): Reg2Insn {
            return Reg2Insn(
                reader.readU16(),
                reader.readU16(),
                reader.readU16()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): Reg2Insn {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): Reg2Insn {
            return Reg2Insn(
                buffer.getShort(offset).toUShort(),
                buffer.getShort(offset + 2).toUShort(),
                buffer.getShort(offset + 4).toUShort()
            )
        }
    }
}


data class Reg3Insn(
    val opcode: UShort,
    val regA: UShort,
    val regB: UShort,
    val regC: UShort
) {
    internal fun wireSize(): Int {
        return 8
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(opcode)
        writer.writeU16(regA)
        writer.writeU16(regB)
        writer.writeU16(regC)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putShort(offset, opcode.toShort())
        buffer.putShort(offset + 2, regA.toShort())
        buffer.putShort(offset + 4, regB.toShort())
        buffer.putShort(offset + 6, regC.toShort())
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 8
        internal fun fromReader(reader: WireReader): Reg3Insn {
            return Reg3Insn(
                reader.readU16(),
                reader.readU16(),
                reader.readU16(),
                reader.readU16()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): Reg3Insn {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): Reg3Insn {
            return Reg3Insn(
                buffer.getShort(offset).toUShort(),
                buffer.getShort(offset + 2).toUShort(),
                buffer.getShort(offset + 4).toUShort(),
                buffer.getShort(offset + 6).toUShort()
            )
        }
    }
}


data class RegLiteralInsn(
    val opcode: UShort,
    val regA: UShort,
    val regB: UShort,
    val literal: Long
) {
    internal fun wireSize(): Int {
        return 14
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(opcode)
        writer.writeU16(regA)
        writer.writeU16(regB)
        writer.writeI64(literal)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putShort(offset, opcode.toShort())
        buffer.putShort(offset + 2, regA.toShort())
        buffer.putShort(offset + 4, regB.toShort())
        buffer.putLong(offset + 8, literal)
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 16
        internal fun fromReader(reader: WireReader): RegLiteralInsn {
            return RegLiteralInsn(
                reader.readU16(),
                reader.readU16(),
                reader.readU16(),
                reader.readI64()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): RegLiteralInsn {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): RegLiteralInsn {
            return RegLiteralInsn(
                buffer.getShort(offset).toUShort(),
                buffer.getShort(offset + 2).toUShort(),
                buffer.getShort(offset + 4).toUShort(),
                buffer.getLong(offset + 8)
            )
        }
    }
}


data class RegStringInsn(
    val opcode: UShort,
    val regA: UShort,
    val value: String
) {
    internal fun wireSize(): Int {
        return 2 + 2 + 4 + Utf8Codec.maxBytes(this.value)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(this.opcode)
        writer.writeU16(this.regA)
        writer.writeString(this.value)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): RegStringInsn {
            return RegStringInsn(
                reader.readU16(),
                reader.readU16(),
                reader.readString()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): RegStringInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class RegTypeInsn(
    val opcode: UShort,
    val regA: UShort,
    val regB: UShort,
    val typeDescriptor: String
) {
    internal fun wireSize(): Int {
        return 2 + 2 + 2 + 4 + Utf8Codec.maxBytes(this.typeDescriptor)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(this.opcode)
        writer.writeU16(this.regA)
        writer.writeU16(this.regB)
        writer.writeString(this.typeDescriptor)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): RegTypeInsn {
            return RegTypeInsn(
                reader.readU16(),
                reader.readU16(),
                reader.readU16(),
                reader.readString()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): RegTypeInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class RegFieldInsn(
    val opcode: UShort,
    val regA: UShort,
    val regB: UShort,
    val `field`: FieldRef
) {
    internal fun wireSize(): Int {
        return 2 + 2 + 2 + this.`field`.wireSize()
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(this.opcode)
        writer.writeU16(this.regA)
        writer.writeU16(this.regB)
        this.`field`.writeTo(writer)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): RegFieldInsn {
            return RegFieldInsn(
                reader.readU16(),
                reader.readU16(),
                reader.readU16(),
                FieldRef.fromReader(reader)
            )
        }

        internal fun fromByteArray(bytes: ByteArray): RegFieldInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class InvokeInsn(
    val opcode: UShort,
    val registers: UShortArray,
    val method: MethodRef
) {
    internal fun wireSize(): Int {
        return 2 + 4 + this.registers.size * 2 + this.method.wireSize()
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(this.opcode)
        writer.writeUShortArray(this.registers)
        this.method.writeTo(writer)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): InvokeInsn {
            return InvokeInsn(
                reader.readU16(),
                reader.readUShortArray(),
                MethodRef.fromReader(reader)
            )
        }

        internal fun fromByteArray(bytes: ByteArray): InvokeInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class InvokeRangeInsn(
    val opcode: UShort,
    val startReg: UShort,
    val regCount: UShort,
    val method: MethodRef
) {
    internal fun wireSize(): Int {
        return 2 + 2 + 2 + this.method.wireSize()
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(this.opcode)
        writer.writeU16(this.startReg)
        writer.writeU16(this.regCount)
        this.method.writeTo(writer)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): InvokeRangeInsn {
            return InvokeRangeInsn(
                reader.readU16(),
                reader.readU16(),
                reader.readU16(),
                MethodRef.fromReader(reader)
            )
        }

        internal fun fromByteArray(bytes: ByteArray): InvokeRangeInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class Branch0Insn(
    val opcode: UShort,
    val offset: Int
) {
    internal fun wireSize(): Int {
        return 6
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(opcode)
        writer.writeI32(offset)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putShort(offset, opcode.toShort())
        buffer.putInt(offset + 4, offset)
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 8
        internal fun fromReader(reader: WireReader): Branch0Insn {
            return Branch0Insn(
                reader.readU16(),
                reader.readI32()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): Branch0Insn {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): Branch0Insn {
            return Branch0Insn(
                buffer.getShort(offset).toUShort(),
                buffer.getInt(offset + 4)
            )
        }
    }
}


data class BranchInsn(
    val opcode: UShort,
    val regA: UShort,
    val offset: Int
) {
    internal fun wireSize(): Int {
        return 8
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(opcode)
        writer.writeU16(regA)
        writer.writeI32(offset)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putShort(offset, opcode.toShort())
        buffer.putShort(offset + 2, regA.toShort())
        buffer.putInt(offset + 4, offset)
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 8
        internal fun fromReader(reader: WireReader): BranchInsn {
            return BranchInsn(
                reader.readU16(),
                reader.readU16(),
                reader.readI32()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): BranchInsn {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): BranchInsn {
            return BranchInsn(
                buffer.getShort(offset).toUShort(),
                buffer.getShort(offset + 2).toUShort(),
                buffer.getInt(offset + 4)
            )
        }
    }
}


data class Branch2Insn(
    val opcode: UShort,
    val regA: UShort,
    val regB: UShort,
    val offset: Int
) {
    internal fun wireSize(): Int {
        return 10
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(opcode)
        writer.writeU16(regA)
        writer.writeU16(regB)
        writer.writeI32(offset)
    }
    internal fun toByteArray(): ByteArray {
        val buffer = java.nio.ByteBuffer
            .allocate(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer.array()
    }

    internal fun toDirectBuffer(): java.nio.ByteBuffer {
        val buffer = java.nio.ByteBuffer
            .allocateDirect(STRUCT_SIZE)
            .order(java.nio.ByteOrder.nativeOrder())
        writeTo(buffer, 0)
        return buffer
    }

    internal fun writeTo(buffer: java.nio.ByteBuffer, offset: Int) {
        buffer.putShort(offset, opcode.toShort())
        buffer.putShort(offset + 2, regA.toShort())
        buffer.putShort(offset + 4, regB.toShort())
        buffer.putInt(offset + 8, offset)
    }

    companion object {
        internal const val STRUCT_SIZE: Int = 12
        internal fun fromReader(reader: WireReader): Branch2Insn {
            return Branch2Insn(
                reader.readU16(),
                reader.readU16(),
                reader.readU16(),
                reader.readI32()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): Branch2Insn {
            require(bytes.size == STRUCT_SIZE)
            val buffer = java.nio.ByteBuffer
                .wrap(bytes)
                .order(java.nio.ByteOrder.nativeOrder())
            return fromBuffer(buffer, 0)
        }

        internal fun fromBuffer(buffer: java.nio.ByteBuffer, offset: Int): Branch2Insn {
            return Branch2Insn(
                buffer.getShort(offset).toUShort(),
                buffer.getShort(offset + 2).toUShort(),
                buffer.getShort(offset + 4).toUShort(),
                buffer.getInt(offset + 8)
            )
        }
    }
}


data class FilledArrayInsn(
    val opcode: UShort,
    val registers: UShortArray,
    val typeDescriptor: String
) {
    internal fun wireSize(): Int {
        return 2 + 4 + this.registers.size * 2 + 4 + Utf8Codec.maxBytes(this.typeDescriptor)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(this.opcode)
        writer.writeUShortArray(this.registers)
        writer.writeString(this.typeDescriptor)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): FilledArrayInsn {
            return FilledArrayInsn(
                reader.readU16(),
                reader.readUShortArray(),
                reader.readString()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): FilledArrayInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class FilledArrayRangeInsn(
    val opcode: UShort,
    val startReg: UShort,
    val regCount: UShort,
    val typeDescriptor: String
) {
    internal fun wireSize(): Int {
        return 2 + 2 + 2 + 4 + Utf8Codec.maxBytes(this.typeDescriptor)
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(this.opcode)
        writer.writeU16(this.startReg)
        writer.writeU16(this.regCount)
        writer.writeString(this.typeDescriptor)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): FilledArrayRangeInsn {
            return FilledArrayRangeInsn(
                reader.readU16(),
                reader.readU16(),
                reader.readU16(),
                reader.readString()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): FilledArrayRangeInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class PackedSwitchInsn(
    val firstKey: Int,
    val targets: IntArray
) {
    internal fun wireSize(): Int {
        return 4 + 4 + this.targets.size * 4
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeI32(this.firstKey)
        writer.writeIntArray(this.targets)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): PackedSwitchInsn {
            return PackedSwitchInsn(
                reader.readI32(),
                reader.readIntArray()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): PackedSwitchInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class SparseSwitchInsn(
    val keys: IntArray,
    val targets: IntArray
) {
    internal fun wireSize(): Int {
        return 4 + this.keys.size * 4 + 4 + this.targets.size * 4
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeIntArray(this.keys)
        writer.writeIntArray(this.targets)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): SparseSwitchInsn {
            return SparseSwitchInsn(
                reader.readIntArray(),
                reader.readIntArray()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): SparseSwitchInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


data class FillArrayInsn(
    val elementWidth: UShort,
    val `data`: ByteArray
) {
    internal fun wireSize(): Int {
        return 2 + 4 + this.`data`.size
    }

    internal fun writeTo(writer: WireWriter) {
        writer.writeU16(this.elementWidth)
        writer.writeBytes(this.`data`)
    }

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): FillArrayInsn {
            return FillArrayInsn(
                reader.readU16(),
                reader.readBytes()
            )
        }

        internal fun fromByteArray(bytes: ByteArray): FillArrayInsn {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


sealed class EncodedVal {
    internal abstract fun wireSize(): Int

    internal abstract fun writeTo(writer: WireWriter)

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }


    object Null : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(0.toUInt())
        }
    }
    data class BoolVal(
        val field0: Boolean
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 1
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(1.toUInt())
            writer.writeBool(this.field0)
        }
    }
    data class ByteVal(
        val field0: Byte
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 1
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(2.toUInt())
            writer.writeI8(this.field0)
        }
    }
    data class ShortVal(
        val field0: Short
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 2
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(3.toUInt())
            writer.writeI16(this.field0)
        }
    }
    data class CharVal(
        val field0: UShort
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 2
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(4.toUInt())
            writer.writeU16(this.field0)
        }
    }
    data class IntVal(
        val field0: Int
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 4
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(5.toUInt())
            writer.writeI32(this.field0)
        }
    }
    data class LongVal(
        val field0: Long
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 8
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(6.toUInt())
            writer.writeI64(this.field0)
        }
    }
    data class FloatVal(
        val field0: Float
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 4
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(7.toUInt())
            writer.writeF32(this.field0)
        }
    }
    data class DoubleVal(
        val field0: Double
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 8
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(8.toUInt())
            writer.writeF64(this.field0)
        }
    }
    data class StringVal(
        val field0: String
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 4 + Utf8Codec.maxBytes(this.field0)
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(9.toUInt())
            writer.writeString(this.field0)
        }
    }
    data class TypeVal(
        val field0: String
    ) : EncodedVal() {
        internal override fun wireSize(): Int {
            return 4 + 4 + Utf8Codec.maxBytes(this.field0)
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(10.toUInt())
            writer.writeString(this.field0)
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): EncodedVal {
            val tag = reader.readU32()
            return when (tag) {
                0.toUInt() -> Null
                1.toUInt() -> BoolVal(reader.readBool())
                2.toUInt() -> ByteVal(reader.readI8())
                3.toUInt() -> ShortVal(reader.readI16())
                4.toUInt() -> CharVal(reader.readU16())
                5.toUInt() -> IntVal(reader.readI32())
                6.toUInt() -> LongVal(reader.readI64())
                7.toUInt() -> FloatVal(reader.readF32())
                8.toUInt() -> DoubleVal(reader.readF64())
                9.toUInt() -> StringVal(reader.readString())
                10.toUInt() -> TypeVal(reader.readString())
                else -> throw IllegalArgumentException("unknown EncodedVal tag: $tag")
            }
        }

        internal fun fromByteArray(bytes: ByteArray): EncodedVal {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}


sealed class Instruction {
    internal abstract fun wireSize(): Int

    internal abstract fun writeTo(writer: WireWriter)

    internal fun toByteArray(): ByteArray {
        val buffer = WireWriterPool.acquire(wireSize())
        val writer = buffer.writer
        try {
            writeTo(writer)
            return buffer.bytes()
        } finally {
            buffer.close()
        }
    }


    data class Simple(
        val field0: app.reseam.patch.SimpleInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(0.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class Reg1(
        val field0: app.reseam.patch.Reg1Insn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(1.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class Reg2(
        val field0: app.reseam.patch.Reg2Insn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(2.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class Reg3(
        val field0: app.reseam.patch.Reg3Insn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(3.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class RegLiteral(
        val field0: app.reseam.patch.RegLiteralInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(4.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class RegString(
        val field0: app.reseam.patch.RegStringInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(5.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class RegType(
        val field0: app.reseam.patch.RegTypeInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(6.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class RegField(
        val field0: app.reseam.patch.RegFieldInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(7.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class Invoke(
        val field0: app.reseam.patch.InvokeInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(8.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class InvokeRange(
        val field0: app.reseam.patch.InvokeRangeInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(9.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class Branch0(
        val field0: app.reseam.patch.Branch0Insn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(10.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class Branch(
        val field0: app.reseam.patch.BranchInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(11.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class Branch2(
        val field0: app.reseam.patch.Branch2Insn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(12.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class FilledArray(
        val field0: app.reseam.patch.FilledArrayInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(13.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class FilledArrayRange(
        val field0: app.reseam.patch.FilledArrayRangeInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(14.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class PackedSwitchData(
        val field0: app.reseam.patch.PackedSwitchInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(15.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class SparseSwitchData(
        val field0: app.reseam.patch.SparseSwitchInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(16.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class FillArrayData(
        val field0: app.reseam.patch.FillArrayInsn
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + this.field0.wireSize()
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(17.toUInt())
            this.field0.writeTo(writer)
        }
    }
    data class Raw(
        val field0: ByteArray
    ) : Instruction() {
        internal override fun wireSize(): Int {
            return 4 + 4 + this.field0.size
        }

        internal override fun writeTo(writer: WireWriter) {
            writer.writeU32(18.toUInt())
            writer.writeBytes(this.field0)
        }
    }

    companion object {
        internal fun fromReader(reader: WireReader): Instruction {
            val tag = reader.readU32()
            return when (tag) {
                0.toUInt() -> Simple(app.reseam.patch.SimpleInsn.fromReader(reader))
                1.toUInt() -> Reg1(app.reseam.patch.Reg1Insn.fromReader(reader))
                2.toUInt() -> Reg2(app.reseam.patch.Reg2Insn.fromReader(reader))
                3.toUInt() -> Reg3(app.reseam.patch.Reg3Insn.fromReader(reader))
                4.toUInt() -> RegLiteral(app.reseam.patch.RegLiteralInsn.fromReader(reader))
                5.toUInt() -> RegString(app.reseam.patch.RegStringInsn.fromReader(reader))
                6.toUInt() -> RegType(app.reseam.patch.RegTypeInsn.fromReader(reader))
                7.toUInt() -> RegField(app.reseam.patch.RegFieldInsn.fromReader(reader))
                8.toUInt() -> Invoke(app.reseam.patch.InvokeInsn.fromReader(reader))
                9.toUInt() -> InvokeRange(app.reseam.patch.InvokeRangeInsn.fromReader(reader))
                10.toUInt() -> Branch0(app.reseam.patch.Branch0Insn.fromReader(reader))
                11.toUInt() -> Branch(app.reseam.patch.BranchInsn.fromReader(reader))
                12.toUInt() -> Branch2(app.reseam.patch.Branch2Insn.fromReader(reader))
                13.toUInt() -> FilledArray(app.reseam.patch.FilledArrayInsn.fromReader(reader))
                14.toUInt() -> FilledArrayRange(app.reseam.patch.FilledArrayRangeInsn.fromReader(reader))
                15.toUInt() -> PackedSwitchData(app.reseam.patch.PackedSwitchInsn.fromReader(reader))
                16.toUInt() -> SparseSwitchData(app.reseam.patch.SparseSwitchInsn.fromReader(reader))
                17.toUInt() -> FillArrayData(app.reseam.patch.FillArrayInsn.fromReader(reader))
                18.toUInt() -> Raw(reader.readBytes())
                else -> throw IllegalArgumentException("unknown Instruction tag: $tag")
            }
        }

        internal fun fromByteArray(bytes: ByteArray): Instruction {
            val reader = WireReader(bytes)
            return fromReader(reader)
        }
    }
}

internal fun setClassAccessFlags(c: UInt, flags: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_class_access_flags(c.toInt(), flags.toInt())
}

internal fun setSuperclass(c: UInt, superclass: String) {
    val __boltffi_superclass_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(superclass))
    val __boltffi_superclass_writer = __boltffi_superclass_wire.writer
    __boltffi_superclass_writer.writeString(superclass)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_superclass(c.toInt(), __boltffi_superclass_wire.directBuffer(), __boltffi_superclass_wire.size())
    } finally {
        __boltffi_superclass_wire.close()
    }
}

internal fun addInterface(c: UInt, interfaceDescriptor: String) {
    val __boltffi_interfaceDescriptor_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(interfaceDescriptor))
    val __boltffi_interfaceDescriptor_writer = __boltffi_interfaceDescriptor_wire.writer
    __boltffi_interfaceDescriptor_writer.writeString(interfaceDescriptor)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_interface(c.toInt(), __boltffi_interfaceDescriptor_wire.directBuffer(), __boltffi_interfaceDescriptor_wire.size())
    } finally {
        __boltffi_interfaceDescriptor_wire.close()
    }
}

internal fun removeClass(c: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_remove_class(c.toInt())
}

/**
 * A new empty class in DEX `dex_index`; 0 when creation fails.
 */
internal fun createClass(dexIndex: UInt, descriptor: String, flags: UInt, superclass: String): UInt {
    val __boltffi_descriptor_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(descriptor))
    val __boltffi_descriptor_writer = __boltffi_descriptor_wire.writer
    __boltffi_descriptor_writer.writeString(descriptor)
    val __boltffi_superclass_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(superclass))
    val __boltffi_superclass_writer = __boltffi_superclass_wire.writer
    __boltffi_superclass_writer.writeString(superclass)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_create_class(dexIndex.toInt(), __boltffi_descriptor_wire.directBuffer(), __boltffi_descriptor_wire.size(), flags.toInt(), __boltffi_superclass_wire.directBuffer(), __boltffi_superclass_wire.size()).toUInt()
    } finally {
        __boltffi_descriptor_wire.close()
        __boltffi_superclass_wire.close()
    }
}

internal fun definalClass(c: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_definal_class(c.toInt())
}

internal fun superclassChain(c: UInt): UIntArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_superclass_chain(c.toInt()) ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUIntArray(__boltffi_result)
}

/**
 * Adds a method; static, constructor and private methods are direct, the
 * rest virtual. Returns its handle, or 0 when the class cannot take it.
 */
internal fun addMethod(c: UInt, method: NewMethod): UInt {
    val __boltffi_method_wire = WireWriterPool.acquire(method.wireSize())
    val __boltffi_method_writer = __boltffi_method_wire.writer
    method.writeTo(__boltffi_method_writer)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_method(c.toInt(), __boltffi_method_wire.directBuffer(), __boltffi_method_wire.size()).toUInt()
    } finally {
        __boltffi_method_wire.close()
    }
}

internal fun removeMethod(m: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_remove_method(m.toInt())
}

internal fun setMethodAccessFlags(m: UInt, flags: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_method_access_flags(m.toInt(), flags.toInt())
}

/**
 * A copy of the method in the same class, under `new_name` when given.
 */
internal fun cloneMethod(m: UInt, newName: String?): UInt {
    val __boltffi_newName_wire = WireWriterPool.acquire(1 + (newName?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_newName_writer = __boltffi_newName_wire.writer
    __boltffi_newName_writer.writeOptionalValue(newName, { __boltffi_newName_writer, __boltffi_value_0 -> __boltffi_newName_writer.writeString(__boltffi_value_0) })
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_clone_method(m.toInt(), __boltffi_newName_wire.directBuffer(), __boltffi_newName_wire.size()).toUInt()
    } finally {
        __boltffi_newName_wire.close()
    }
}

/**
 * Adds a field; a static field's `initial_value` becomes its static value.
 */
internal fun addField(c: UInt, `field`: NewField) {
    val __boltffi_field_wire = WireWriterPool.acquire(`field`.wireSize())
    val __boltffi_field_writer = __boltffi_field_wire.writer
    `field`.writeTo(__boltffi_field_writer)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_field(c.toInt(), __boltffi_field_wire.directBuffer(), __boltffi_field_wire.size())
    } finally {
        __boltffi_field_wire.close()
    }
}

internal fun removeField(c: UInt, name: String) {
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_remove_field(c.toInt(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size())
    } finally {
        __boltffi_name_wire.close()
    }
}

internal fun setFieldAccessFlags(c: UInt, fieldName: String, flags: UInt) {
    val __boltffi_fieldName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(fieldName))
    val __boltffi_fieldName_writer = __boltffi_fieldName_wire.writer
    __boltffi_fieldName_writer.writeString(fieldName)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_field_access_flags(c.toInt(), __boltffi_fieldName_wire.directBuffer(), __boltffi_fieldName_wire.size(), flags.toInt())
    } finally {
        __boltffi_fieldName_wire.close()
    }
}

internal fun setStaticFieldValue(c: UInt, fieldName: String, value: EncodedVal) {
    val __boltffi_fieldName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(fieldName))
    val __boltffi_fieldName_writer = __boltffi_fieldName_wire.writer
    __boltffi_fieldName_writer.writeString(fieldName)
    val __boltffi_value_wire = WireWriterPool.acquire(value.wireSize())
    val __boltffi_value_writer = __boltffi_value_wire.writer
    value.writeTo(__boltffi_value_writer)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_set_static_field_value(c.toInt(), __boltffi_fieldName_wire.directBuffer(), __boltffi_fieldName_wire.size(), __boltffi_value_wire.directBuffer(), __boltffi_value_wire.size())
    } finally {
        __boltffi_fieldName_wire.close()
        __boltffi_value_wire.close()
    }
}

internal fun addClassAnnotation(c: UInt, `annotation`: AnnotationItem) {
    val __boltffi_annotation_wire = WireWriterPool.acquire(`annotation`.wireSize())
    val __boltffi_annotation_writer = __boltffi_annotation_wire.writer
    `annotation`.writeTo(__boltffi_annotation_writer)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_class_annotation(c.toInt(), __boltffi_annotation_wire.directBuffer(), __boltffi_annotation_wire.size())
    } finally {
        __boltffi_annotation_wire.close()
    }
}

internal fun addMethodAnnotation(m: UInt, `annotation`: AnnotationItem) {
    val __boltffi_annotation_wire = WireWriterPool.acquire(`annotation`.wireSize())
    val __boltffi_annotation_writer = __boltffi_annotation_wire.writer
    `annotation`.writeTo(__boltffi_annotation_writer)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_method_annotation(m.toInt(), __boltffi_annotation_wire.directBuffer(), __boltffi_annotation_wire.size())
    } finally {
        __boltffi_annotation_wire.close()
    }
}

internal fun addFieldAnnotation(c: UInt, fieldName: String, `annotation`: AnnotationItem) {
    val __boltffi_fieldName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(fieldName))
    val __boltffi_fieldName_writer = __boltffi_fieldName_wire.writer
    __boltffi_fieldName_writer.writeString(fieldName)
    val __boltffi_annotation_wire = WireWriterPool.acquire(`annotation`.wireSize())
    val __boltffi_annotation_writer = __boltffi_annotation_wire.writer
    `annotation`.writeTo(__boltffi_annotation_writer)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_add_field_annotation(c.toInt(), __boltffi_fieldName_wire.directBuffer(), __boltffi_fieldName_wire.size(), __boltffi_annotation_wire.directBuffer(), __boltffi_annotation_wire.size())
    } finally {
        __boltffi_fieldName_wire.close()
        __boltffi_annotation_wire.close()
    }
}

internal fun dexCount(): UInt {
    return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_dex_count().toUInt()
}

internal fun methodDex(m: UInt): UInt {
    return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_method_dex(m.toInt()).toUInt()
}

internal fun internString(d: UInt, s: String): UInt {
    val __boltffi_s_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(s))
    val __boltffi_s_writer = __boltffi_s_wire.writer
    __boltffi_s_writer.writeString(s)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_string(d.toInt(), __boltffi_s_wire.directBuffer(), __boltffi_s_wire.size()).toUInt()
    } finally {
        __boltffi_s_wire.close()
    }
}

internal fun internType(d: UInt, descriptor: String): UInt {
    val __boltffi_descriptor_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(descriptor))
    val __boltffi_descriptor_writer = __boltffi_descriptor_wire.writer
    __boltffi_descriptor_writer.writeString(descriptor)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_type(d.toInt(), __boltffi_descriptor_wire.directBuffer(), __boltffi_descriptor_wire.size()).toUInt()
    } finally {
        __boltffi_descriptor_wire.close()
    }
}

internal fun internProto(d: UInt, proto: String): UInt {
    val __boltffi_proto_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(proto))
    val __boltffi_proto_writer = __boltffi_proto_wire.writer
    __boltffi_proto_writer.writeString(proto)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_proto(d.toInt(), __boltffi_proto_wire.directBuffer(), __boltffi_proto_wire.size()).toUInt()
    } finally {
        __boltffi_proto_wire.close()
    }
}

internal fun internMethod(d: UInt, descriptor: String, name: String, proto: String): UInt {
    val __boltffi_descriptor_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(descriptor))
    val __boltffi_descriptor_writer = __boltffi_descriptor_wire.writer
    __boltffi_descriptor_writer.writeString(descriptor)
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    val __boltffi_proto_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(proto))
    val __boltffi_proto_writer = __boltffi_proto_wire.writer
    __boltffi_proto_writer.writeString(proto)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_method(d.toInt(), __boltffi_descriptor_wire.directBuffer(), __boltffi_descriptor_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size(), __boltffi_proto_wire.directBuffer(), __boltffi_proto_wire.size()).toUInt()
    } finally {
        __boltffi_descriptor_wire.close()
        __boltffi_name_wire.close()
        __boltffi_proto_wire.close()
    }
}

internal fun internField(d: UInt, descriptor: String, name: String, fieldType: String): UInt {
    val __boltffi_descriptor_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(descriptor))
    val __boltffi_descriptor_writer = __boltffi_descriptor_wire.writer
    __boltffi_descriptor_writer.writeString(descriptor)
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    val __boltffi_fieldType_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(fieldType))
    val __boltffi_fieldType_writer = __boltffi_fieldType_wire.writer
    __boltffi_fieldType_writer.writeString(fieldType)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_intern_field(d.toInt(), __boltffi_descriptor_wire.directBuffer(), __boltffi_descriptor_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size(), __boltffi_fieldType_wire.directBuffer(), __boltffi_fieldType_wire.size()).toUInt()
    } finally {
        __boltffi_descriptor_wire.close()
        __boltffi_name_wire.close()
        __boltffi_fieldType_wire.close()
    }
}

internal fun findStringIdx(d: UInt, s: String): UInt? {
    val __boltffi_s_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(s))
    val __boltffi_s_writer = __boltffi_s_wire.writer
    __boltffi_s_writer.writeString(s)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_find_string_idx(d.toInt(), __boltffi_s_wire.directBuffer(), __boltffi_s_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_s_wire.close()
    }
}

internal fun getString(d: UInt, idx: UInt): String {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_get_string(d.toInt(), idx.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readString()
}

internal fun getTypeDescriptor(d: UInt, idx: UInt): String {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_get_type_descriptor(d.toInt(), idx.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readString()
}

internal fun buildLookups(d: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_class_ops_build_lookups(d.toInt())
}

internal fun findMethod(classDescriptor: String, methodName: String): UInt? {
    val __boltffi_classDescriptor_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(classDescriptor))
    val __boltffi_classDescriptor_writer = __boltffi_classDescriptor_wire.writer
    __boltffi_classDescriptor_writer.writeString(classDescriptor)
    val __boltffi_methodName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(methodName))
    val __boltffi_methodName_writer = __boltffi_methodName_wire.writer
    __boltffi_methodName_writer.writeString(methodName)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_method(__boltffi_classDescriptor_wire.directBuffer(), __boltffi_classDescriptor_wire.size(), __boltffi_methodName_wire.directBuffer(), __boltffi_methodName_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_classDescriptor_wire.close()
        __boltffi_methodName_wire.close()
    }
}

internal fun findMethodByName(name: String): UInt? {
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_method_by_name(__boltffi_name_wire.directBuffer(), __boltffi_name_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_name_wire.close()
    }
}

internal fun findMethodsByName(name: String): UIntArray {
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_name(__boltffi_name_wire.directBuffer(), __boltffi_name_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readUIntArray(__boltffi_result)
    } finally {
        __boltffi_name_wire.close()
    }
}

internal fun findMethodsByStrings(strings: List<String>): UIntArray {
    val __boltffi_strings_wire = WireWriterPool.acquire(4 + strings.sumOf { __boltffi_value_0 -> (4 + Utf8Codec.maxBytes(__boltffi_value_0)).toInt() })
    val __boltffi_strings_writer = __boltffi_strings_wire.writer
    __boltffi_strings_writer.writeSequence(strings, strings.size, { __boltffi_strings_writer, __boltffi_value_0 -> __boltffi_strings_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_strings(__boltffi_strings_wire.directBuffer(), __boltffi_strings_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readUIntArray(__boltffi_result)
    } finally {
        __boltffi_strings_wire.close()
    }
}

/**
 * Methods whose prototype satisfies every given filter: exact return type,
 * exact parameter list, and a parameter of `parameter` type anywhere.
 */
internal fun findMethodsByProto(returnType: String?, parameterTypes: List<String>?, parameter: String?): UIntArray {
    val __boltffi_returnType_wire = WireWriterPool.acquire(1 + (returnType?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_returnType_writer = __boltffi_returnType_wire.writer
    __boltffi_returnType_writer.writeOptionalValue(returnType, { __boltffi_returnType_writer, __boltffi_value_0 -> __boltffi_returnType_writer.writeString(__boltffi_value_0) })
    val __boltffi_parameterTypes_wire = WireWriterPool.acquire(1 + (parameterTypes?.let { __boltffi_value_0 -> 4 + __boltffi_value_0.sumOf { __boltffi_value_1 -> (4 + Utf8Codec.maxBytes(__boltffi_value_1)).toInt() } } ?: 0))
    val __boltffi_parameterTypes_writer = __boltffi_parameterTypes_wire.writer
    __boltffi_parameterTypes_writer.writeOptionalValue(parameterTypes, { __boltffi_parameterTypes_writer, __boltffi_value_0 -> __boltffi_parameterTypes_writer.writeSequence(__boltffi_value_0, __boltffi_value_0.size, { __boltffi_parameterTypes_writer, __boltffi_value_1 -> __boltffi_parameterTypes_writer.writeString(__boltffi_value_1) }) })
    val __boltffi_parameter_wire = WireWriterPool.acquire(1 + (parameter?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_parameter_writer = __boltffi_parameter_wire.writer
    __boltffi_parameter_writer.writeOptionalValue(parameter, { __boltffi_parameter_writer, __boltffi_value_0 -> __boltffi_parameter_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_proto(__boltffi_returnType_wire.directBuffer(), __boltffi_returnType_wire.size(), __boltffi_parameterTypes_wire.directBuffer(), __boltffi_parameterTypes_wire.size(), __boltffi_parameter_wire.directBuffer(), __boltffi_parameter_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readUIntArray(__boltffi_result)
    } finally {
        __boltffi_returnType_wire.close()
        __boltffi_parameterTypes_wire.close()
        __boltffi_parameter_wire.close()
    }
}

/**
 * Negative opcodes match any instruction.
 */
internal fun findMethodsByOpcodes(pattern: IntArray): UIntArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_opcodes(pattern) ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUIntArray(__boltffi_result)
}

internal fun findMethodByFingerprint(fp: FingerprintDef): FingerprintResult? {
    val __boltffi_fp_wire = WireWriterPool.acquire(fp.wireSize())
    val __boltffi_fp_writer = __boltffi_fp_wire.writer
    fp.writeTo(__boltffi_fp_writer)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_method_by_fingerprint(__boltffi_fp_wire.directBuffer(), __boltffi_fp_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> FingerprintResult.fromReader(__boltffi_reader) })
    } finally {
        __boltffi_fp_wire.close()
    }
}

internal fun findMethodsByFingerprint(fp: FingerprintDef): List<FingerprintResult> {
    val __boltffi_fp_wire = WireWriterPool.acquire(fp.wireSize())
    val __boltffi_fp_writer = __boltffi_fp_wire.writer
    fp.writeTo(__boltffi_fp_writer)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_methods_by_fingerprint(__boltffi_fp_wire.directBuffer(), __boltffi_fp_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readRecordList(__boltffi_result, 8, { buffer, offset -> FingerprintResult.fromBuffer(buffer, offset) })
    } finally {
        __boltffi_fp_wire.close()
    }
}

internal fun findClass(descriptor: String): UInt? {
    val __boltffi_descriptor_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(descriptor))
    val __boltffi_descriptor_writer = __boltffi_descriptor_wire.writer
    __boltffi_descriptor_writer.writeString(descriptor)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_find_class(__boltffi_descriptor_wire.directBuffer(), __boltffi_descriptor_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_descriptor_wire.close()
    }
}

internal fun getAllClasses(): UIntArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_get_all_classes() ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUIntArray(__boltffi_result)
}

internal fun getMethodInfo(m: UInt): MethodInfo? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_get_method_info(m.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalValue({ __boltffi_reader -> MethodInfo.fromReader(__boltffi_reader) })
}

internal fun getClassInfo(c: UInt): ClassInfo? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_get_class_info(c.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalValue({ __boltffi_reader -> ClassInfo.fromReader(__boltffi_reader) })
}

internal fun classDirectMethods(c: UInt): UIntArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_class_direct_methods(c.toInt()) ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUIntArray(__boltffi_result)
}

internal fun classVirtualMethods(c: UInt): UIntArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_class_virtual_methods(c.toInt()) ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUIntArray(__boltffi_result)
}

/**
 * Static fields first, then instance fields.
 */
internal fun classFields(c: UInt): List<FieldInfo> {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_lookup_class_fields(c.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readSequence({ __boltffi_reader -> FieldInfo.fromReader(__boltffi_reader) })
}

internal fun setInstructions(m: UInt, insns: List<Instruction>) {
    val __boltffi_insns_wire = WireWriterPool.acquire(4 + insns.sumOf { __boltffi_value_0 -> (__boltffi_value_0.wireSize()).toInt() })
    val __boltffi_insns_writer = __boltffi_insns_wire.writer
    __boltffi_insns_writer.writeSequence(insns, insns.size, { __boltffi_insns_writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(__boltffi_insns_writer) })
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_set_instructions(m.toInt(), __boltffi_insns_wire.directBuffer(), __boltffi_insns_wire.size())
    } finally {
        __boltffi_insns_wire.close()
    }
}

/**
 * Replaces the whole body, dropping debug info the new code cannot match.
 */
internal fun replaceBody(m: UInt, registersSize: UShort, outsSize: UShort, insns: List<Instruction>) {
    val __boltffi_insns_wire = WireWriterPool.acquire(4 + insns.sumOf { __boltffi_value_0 -> (__boltffi_value_0.wireSize()).toInt() })
    val __boltffi_insns_writer = __boltffi_insns_wire.writer
    __boltffi_insns_writer.writeSequence(insns, insns.size, { __boltffi_insns_writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(__boltffi_insns_writer) })
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_body(m.toInt(), registersSize.toShort(), outsSize.toShort(), __boltffi_insns_wire.directBuffer(), __boltffi_insns_wire.size())
    } finally {
        __boltffi_insns_wire.close()
    }
}

internal fun insertInstructions(m: UInt, index: UInt, insns: List<Instruction>) {
    val __boltffi_insns_wire = WireWriterPool.acquire(4 + insns.sumOf { __boltffi_value_0 -> (__boltffi_value_0.wireSize()).toInt() })
    val __boltffi_insns_writer = __boltffi_insns_wire.writer
    __boltffi_insns_writer.writeSequence(insns, insns.size, { __boltffi_insns_writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(__boltffi_insns_writer) })
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_insert_instructions(m.toInt(), index.toInt(), __boltffi_insns_wire.directBuffer(), __boltffi_insns_wire.size())
    } finally {
        __boltffi_insns_wire.close()
    }
}

internal fun insertBeforeInstruction(m: UInt, index: UInt, insns: List<Instruction>): Boolean {
    val __boltffi_insns_wire = WireWriterPool.acquire(4 + insns.sumOf { __boltffi_value_0 -> (__boltffi_value_0.wireSize()).toInt() })
    val __boltffi_insns_writer = __boltffi_insns_wire.writer
    __boltffi_insns_writer.writeSequence(insns, insns.size, { __boltffi_insns_writer, __boltffi_value_0 -> __boltffi_value_0.writeTo(__boltffi_insns_writer) })
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_insert_before_instruction(m.toInt(), index.toInt(), __boltffi_insns_wire.directBuffer(), __boltffi_insns_wire.size())
    } finally {
        __boltffi_insns_wire.close()
    }
}

internal fun replaceInstruction(m: UInt, index: UInt, insn: Instruction) {
    val __boltffi_insn_wire = WireWriterPool.acquire(insn.wireSize())
    val __boltffi_insn_writer = __boltffi_insn_wire.writer
    insn.writeTo(__boltffi_insn_writer)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_instruction(m.toInt(), index.toInt(), __boltffi_insn_wire.directBuffer(), __boltffi_insn_wire.size())
    } finally {
        __boltffi_insn_wire.close()
    }
}

internal fun removeInstructions(m: UInt, index: UInt, count: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_remove_instructions(m.toInt(), index.toInt(), count.toInt())
}

internal fun returnEarly(m: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_return_early(m.toInt())
}

internal fun returnEarlyInt(m: UInt, value: Int) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_return_early_int(m.toInt(), value)
}

internal fun returnEarlyObjectNull(m: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_return_early_object_null(m.toInt())
}

internal fun returnEarlyWide(m: UInt, value: Long) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_return_early_wide(m.toInt(), value)
}

/**
 * Rewrites string constants equal to `old`; `all` decides whether to stop
 * after the first. Returns how many changed.
 */
internal fun replaceStrings(m: UInt, old: String, new: String, all: Boolean): UInt {
    val __boltffi_old_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(old))
    val __boltffi_old_writer = __boltffi_old_wire.writer
    __boltffi_old_writer.writeString(old)
    val __boltffi_new_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(new))
    val __boltffi_new_writer = __boltffi_new_wire.writer
    __boltffi_new_writer.writeString(new)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_strings(m.toInt(), __boltffi_old_wire.directBuffer(), __boltffi_old_wire.size(), __boltffi_new_wire.directBuffer(), __boltffi_new_wire.size(), all).toUInt()
    } finally {
        __boltffi_old_wire.close()
        __boltffi_new_wire.close()
    }
}

/**
 * Rewrites literals equal to `old` where the instruction can encode `new`;
 * `all` decides whether to stop after the first. Returns how many changed.
 */
internal fun replaceLiterals(m: UInt, old: Long, new: Long, all: Boolean): UInt {
    return Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_literals(m.toInt(), old, new, all).toUInt()
}

internal fun replaceMethodCall(m: UInt, index: UInt, newClass: String, newName: String, newProto: String): Boolean {
    val __boltffi_newClass_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(newClass))
    val __boltffi_newClass_writer = __boltffi_newClass_wire.writer
    __boltffi_newClass_writer.writeString(newClass)
    val __boltffi_newName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(newName))
    val __boltffi_newName_writer = __boltffi_newName_wire.writer
    __boltffi_newName_writer.writeString(newName)
    val __boltffi_newProto_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(newProto))
    val __boltffi_newProto_writer = __boltffi_newProto_wire.writer
    __boltffi_newProto_writer.writeString(newProto)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_replace_method_call(m.toInt(), index.toInt(), __boltffi_newClass_wire.directBuffer(), __boltffi_newClass_wire.size(), __boltffi_newName_wire.directBuffer(), __boltffi_newName_wire.size(), __boltffi_newProto_wire.directBuffer(), __boltffi_newProto_wire.size())
    } finally {
        __boltffi_newClass_wire.close()
        __boltffi_newName_wire.close()
        __boltffi_newProto_wire.close()
    }
}

/**
 * Every call to `from` in the app becomes a static call to `to` with the
 * same registers; see `PatchContext::redirect_method_calls`.
 */
internal fun redirectMethodCalls(from: MethodRef, to: MethodRef): UInt {
    val __boltffi_from_wire = WireWriterPool.acquire(from.wireSize())
    val __boltffi_from_writer = __boltffi_from_wire.writer
    from.writeTo(__boltffi_from_writer)
    val __boltffi_to_wire = WireWriterPool.acquire(to.wireSize())
    val __boltffi_to_writer = __boltffi_to_wire.writer
    to.writeTo(__boltffi_to_writer)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_redirect_method_calls(__boltffi_from_wire.directBuffer(), __boltffi_from_wire.size(), __boltffi_to_wire.directBuffer(), __boltffi_to_wire.size()).toUInt()
    } finally {
        __boltffi_from_wire.close()
        __boltffi_to_wire.close()
    }
}

internal fun insertInvokeStatic(m: UInt, index: UInt, className: String, name: String, proto: String, registers: UShortArray): Boolean {
    val __boltffi_className_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(className))
    val __boltffi_className_writer = __boltffi_className_wire.writer
    __boltffi_className_writer.writeString(className)
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    val __boltffi_proto_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(proto))
    val __boltffi_proto_writer = __boltffi_proto_wire.writer
    __boltffi_proto_writer.writeString(proto)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_insert_invoke_static(m.toInt(), index.toInt(), __boltffi_className_wire.directBuffer(), __boltffi_className_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size(), __boltffi_proto_wire.directBuffer(), __boltffi_proto_wire.size(), registers.asShortArray())
    } finally {
        __boltffi_className_wire.close()
        __boltffi_name_wire.close()
        __boltffi_proto_wire.close()
    }
}

/**
 * Inserts a static call followed by a `move-result` into `result_register`.
 */
internal fun insertInvokeStaticWithMoveResult(m: UInt, index: UInt, className: String, name: String, proto: String, registers: UShortArray, resultRegister: UShort, isObject: Boolean): Boolean {
    val __boltffi_className_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(className))
    val __boltffi_className_writer = __boltffi_className_wire.writer
    __boltffi_className_writer.writeString(className)
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    val __boltffi_proto_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(proto))
    val __boltffi_proto_writer = __boltffi_proto_wire.writer
    __boltffi_proto_writer.writeString(proto)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_bytecode_mutation_insert_invoke_static_with_move_result(m.toInt(), index.toInt(), __boltffi_className_wire.directBuffer(), __boltffi_className_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size(), __boltffi_proto_wire.directBuffer(), __boltffi_proto_wire.size(), registers.asShortArray(), resultRegister.toShort(), isObject)
    } finally {
        __boltffi_className_wire.close()
        __boltffi_name_wire.close()
        __boltffi_proto_wire.close()
    }
}

internal fun ensureOutsSize(m: UInt, minOutsSize: UShort) {
    Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_ensure_outs_size(m.toInt(), minOutsSize.toShort())
}

/**
 * Adds locals below the incoming registers and returns relocated instruction
 * indices, including the end boundary. The method is unchanged on failure.
 */
internal fun growLocalRegisters(m: UInt, additionalLocals: UShort): UIntArray? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_grow_local_registers(m.toInt(), additionalLocals.toShort()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readUIntArray() })
}

internal fun registersSize(m: UInt): UShort {
    return Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_registers_size(m.toInt()).toUShort()
}

internal fun insSize(m: UInt): UShort {
    return Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_ins_size(m.toInt()).toUShort()
}

internal fun outsSize(m: UInt): UShort {
    return Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_outs_size(m.toInt()).toUShort()
}

internal fun findFreeRegister(m: UInt, atIndex: UInt, exclude: UShortArray): UShort? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_find_free_register(m.toInt(), atIndex.toInt(), exclude.asShortArray()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalU16()
}

internal fun findFreeRegisters(m: UInt, atIndex: UInt, count: UInt, exclude: UShortArray): UShortArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_find_free_registers(m.toInt(), atIndex.toInt(), count.toInt(), exclude.asShortArray()) ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUShortArray(__boltffi_result)
}

internal fun findContiguousFreeRegisters(m: UInt, atIndex: UInt, count: UInt, exclude: UShortArray): UShortArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_find_contiguous_free_registers(m.toInt(), atIndex.toInt(), count.toInt(), exclude.asShortArray()) ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUShortArray(__boltffi_result)
}

/**
 * The `position`th register operand of the instruction at `index`, or 0.
 */
internal fun instructionRegister(m: UInt, index: UInt, position: UInt): UShort {
    return Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_instruction_register(m.toInt(), index.toInt(), position.toInt()).toUShort()
}

internal fun instructionWideLiteral(m: UInt, index: UInt): Long {
    return Native.boltffi_function_reseam_patcher_kotlin_bytecode_registers_instruction_wide_literal(m.toInt(), index.toInt())
}

internal fun getInstructions(m: UInt): List<Instruction> {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_get_instructions(m.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readSequence({ __boltffi_reader -> Instruction.fromReader(__boltffi_reader) })
}

/**
 * The instruction at `index`, or a `nop` when there is none.
 */
internal fun getInstruction(m: UInt, index: UInt): Instruction {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_get_instruction(m.toInt(), index.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return Instruction.fromReader(__boltffi_reader)
}

internal fun instructionCount(m: UInt): UInt {
    return Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_count(m.toInt()).toUInt()
}

internal fun indexOfFirst(m: UInt, start: UInt, op: UShort): UInt? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first(m.toInt(), start.toInt(), op.toShort()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalU32()
}

internal fun indexOfFirstReversed(m: UInt, start: UInt, op: UShort): UInt? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_reversed(m.toInt(), start.toInt(), op.toShort()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalU32()
}

internal fun indexOfFirstLiteral(m: UInt, literal: Long): UInt? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_literal(m.toInt(), literal) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalU32()
}

internal fun indexOfFirstLiteralReversed(m: UInt, literal: Long): UInt? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_literal_reversed(m.toInt(), literal) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalU32()
}

internal fun indexOfFirstString(m: UInt, s: String): UInt? {
    val __boltffi_s_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(s))
    val __boltffi_s_writer = __boltffi_s_wire.writer
    __boltffi_s_writer.writeString(s)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_string(m.toInt(), __boltffi_s_wire.directBuffer(), __boltffi_s_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_s_wire.close()
    }
}

internal fun findAllIndices(m: UInt, op: UShort): UIntArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_find_all_indices(m.toInt(), op.toShort()) ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUIntArray(__boltffi_result)
}

internal fun indexOfFirstMethodCall(m: UInt, definingClass: String, methodName: String, start: UInt): UInt? {
    val __boltffi_definingClass_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(definingClass))
    val __boltffi_definingClass_writer = __boltffi_definingClass_wire.writer
    __boltffi_definingClass_writer.writeString(definingClass)
    val __boltffi_methodName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(methodName))
    val __boltffi_methodName_writer = __boltffi_methodName_wire.writer
    __boltffi_methodName_writer.writeString(methodName)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_method_call(m.toInt(), __boltffi_definingClass_wire.directBuffer(), __boltffi_definingClass_wire.size(), __boltffi_methodName_wire.directBuffer(), __boltffi_methodName_wire.size(), start.toInt()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_definingClass_wire.close()
        __boltffi_methodName_wire.close()
    }
}

/**
 * A field access matching every given filter; `op` below zero matches any opcode.
 */
internal fun indexOfFirstFieldAccess(m: UInt, op: Int, fieldType: String?, definingClass: String?, start: UInt): UInt? {
    val __boltffi_fieldType_wire = WireWriterPool.acquire(1 + (fieldType?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_fieldType_writer = __boltffi_fieldType_wire.writer
    __boltffi_fieldType_writer.writeOptionalValue(fieldType, { __boltffi_fieldType_writer, __boltffi_value_0 -> __boltffi_fieldType_writer.writeString(__boltffi_value_0) })
    val __boltffi_definingClass_wire = WireWriterPool.acquire(1 + (definingClass?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_definingClass_writer = __boltffi_definingClass_wire.writer
    __boltffi_definingClass_writer.writeOptionalValue(definingClass, { __boltffi_definingClass_writer, __boltffi_value_0 -> __boltffi_definingClass_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_first_field_access(m.toInt(), op, __boltffi_fieldType_wire.directBuffer(), __boltffi_fieldType_wire.size(), __boltffi_definingClass_wire.directBuffer(), __boltffi_definingClass_wire.size(), start.toInt()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_fieldType_wire.close()
        __boltffi_definingClass_wire.close()
    }
}

/**
 * The first index at or after `start` where `opcodes` match consecutively;
 * negative opcodes match anything.
 */
internal fun indexOfOpcodeSequence(m: UInt, opcodes: IntArray, start: UInt): UInt? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_index_of_opcode_sequence(m.toInt(), opcodes, start.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalU32()
}

internal fun findInstructionsByLiteral(literal: Long): List<InstructionHit> {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_literal(literal) ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readRecordList(__boltffi_result, 8, { buffer, offset -> InstructionHit.fromBuffer(buffer, offset) })
}

internal fun findInstructionsByString(s: String): List<InstructionHit> {
    val __boltffi_s_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(s))
    val __boltffi_s_writer = __boltffi_s_wire.writer
    __boltffi_s_writer.writeString(s)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_string(__boltffi_s_wire.directBuffer(), __boltffi_s_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readRecordList(__boltffi_result, 8, { buffer, offset -> InstructionHit.fromBuffer(buffer, offset) })
    } finally {
        __boltffi_s_wire.close()
    }
}

internal fun findInstructionsByStringContains(substring: String): List<InstructionHit> {
    val __boltffi_substring_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(substring))
    val __boltffi_substring_writer = __boltffi_substring_wire.writer
    __boltffi_substring_writer.writeString(substring)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_string_contains(__boltffi_substring_wire.directBuffer(), __boltffi_substring_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readRecordList(__boltffi_result, 8, { buffer, offset -> InstructionHit.fromBuffer(buffer, offset) })
    } finally {
        __boltffi_substring_wire.close()
    }
}

internal fun findInstructionsByResourceId(resType: String, resName: String): List<InstructionHit> {
    val __boltffi_resType_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resType))
    val __boltffi_resType_writer = __boltffi_resType_wire.writer
    __boltffi_resType_writer.writeString(resType)
    val __boltffi_resName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resName))
    val __boltffi_resName_writer = __boltffi_resName_wire.writer
    __boltffi_resName_writer.writeString(resName)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_resource_id(__boltffi_resType_wire.directBuffer(), __boltffi_resType_wire.size(), __boltffi_resName_wire.directBuffer(), __boltffi_resName_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readRecordList(__boltffi_result, 8, { buffer, offset -> InstructionHit.fromBuffer(buffer, offset) })
    } finally {
        __boltffi_resType_wire.close()
        __boltffi_resName_wire.close()
    }
}

/**
 * Call sites of `(class_names[i], method_names[i])` pairs.
 */
internal fun findMethodCallSites(classNames: List<String>, methodNames: List<String>): List<MethodCallSiteResult> {
    val __boltffi_classNames_wire = WireWriterPool.acquire(4 + classNames.sumOf { __boltffi_value_0 -> (4 + Utf8Codec.maxBytes(__boltffi_value_0)).toInt() })
    val __boltffi_classNames_writer = __boltffi_classNames_wire.writer
    __boltffi_classNames_writer.writeSequence(classNames, classNames.size, { __boltffi_classNames_writer, __boltffi_value_0 -> __boltffi_classNames_writer.writeString(__boltffi_value_0) })
    val __boltffi_methodNames_wire = WireWriterPool.acquire(4 + methodNames.sumOf { __boltffi_value_0 -> (4 + Utf8Codec.maxBytes(__boltffi_value_0)).toInt() })
    val __boltffi_methodNames_writer = __boltffi_methodNames_wire.writer
    __boltffi_methodNames_writer.writeSequence(methodNames, methodNames.size, { __boltffi_methodNames_writer, __boltffi_value_0 -> __boltffi_methodNames_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_find_method_call_sites(__boltffi_classNames_wire.directBuffer(), __boltffi_classNames_wire.size(), __boltffi_methodNames_wire.directBuffer(), __boltffi_methodNames_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readRecordList(__boltffi_result, 12, { buffer, offset -> MethodCallSiteResult.fromBuffer(buffer, offset) })
    } finally {
        __boltffi_classNames_wire.close()
        __boltffi_methodNames_wire.close()
    }
}

/**
 * Accesses of `(class_names[i], field_names[i])` pairs.
 */
internal fun findFieldAccessSites(classNames: List<String>, fieldNames: List<String>): List<MethodCallSiteResult> {
    val __boltffi_classNames_wire = WireWriterPool.acquire(4 + classNames.sumOf { __boltffi_value_0 -> (4 + Utf8Codec.maxBytes(__boltffi_value_0)).toInt() })
    val __boltffi_classNames_writer = __boltffi_classNames_wire.writer
    __boltffi_classNames_writer.writeSequence(classNames, classNames.size, { __boltffi_classNames_writer, __boltffi_value_0 -> __boltffi_classNames_writer.writeString(__boltffi_value_0) })
    val __boltffi_fieldNames_wire = WireWriterPool.acquire(4 + fieldNames.sumOf { __boltffi_value_0 -> (4 + Utf8Codec.maxBytes(__boltffi_value_0)).toInt() })
    val __boltffi_fieldNames_writer = __boltffi_fieldNames_wire.writer
    __boltffi_fieldNames_writer.writeSequence(fieldNames, fieldNames.size, { __boltffi_fieldNames_writer, __boltffi_value_0 -> __boltffi_fieldNames_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_find_field_access_sites(__boltffi_classNames_wire.directBuffer(), __boltffi_classNames_wire.size(), __boltffi_fieldNames_wire.directBuffer(), __boltffi_fieldNames_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readRecordList(__boltffi_result, 12, { buffer, offset -> MethodCallSiteResult.fromBuffer(buffer, offset) })
    } finally {
        __boltffi_classNames_wire.close()
        __boltffi_fieldNames_wire.close()
    }
}

internal fun findInstructionsByInvoke(definingClass: String, methodName: String): List<InstructionHit> {
    val __boltffi_definingClass_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(definingClass))
    val __boltffi_definingClass_writer = __boltffi_definingClass_wire.writer
    __boltffi_definingClass_writer.writeString(definingClass)
    val __boltffi_methodName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(methodName))
    val __boltffi_methodName_writer = __boltffi_methodName_wire.writer
    __boltffi_methodName_writer.writeString(methodName)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_find_instructions_by_invoke(__boltffi_definingClass_wire.directBuffer(), __boltffi_definingClass_wire.size(), __boltffi_methodName_wire.directBuffer(), __boltffi_methodName_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readRecordList(__boltffi_result, 8, { buffer, offset -> InstructionHit.fromBuffer(buffer, offset) })
    } finally {
        __boltffi_definingClass_wire.close()
        __boltffi_methodName_wire.close()
    }
}

internal fun allMethodHandles(): UIntArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_all_method_handles() ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUIntArray(__boltffi_result)
}

internal fun instructionStringRef(m: UInt, index: UInt): String? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_string_ref(m.toInt(), index.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
}

internal fun instructionMethodRef(m: UInt, index: UInt): MethodRef? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_method_ref(m.toInt(), index.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalValue({ __boltffi_reader -> MethodRef.fromReader(__boltffi_reader) })
}

internal fun instructionFieldRef(m: UInt, index: UInt): FieldRef? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_field_ref(m.toInt(), index.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalValue({ __boltffi_reader -> FieldRef.fromReader(__boltffi_reader) })
}

internal fun instructionTypeRef(m: UInt, index: UInt): String? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_bytecode_search_instruction_type_ref(m.toInt(), index.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
}

internal fun componentNames(): List<String> {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_files_component_names() ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readSequence({ __boltffi_reader -> __boltffi_reader.readString() })
}

internal fun fileList(component: String?): List<String> {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_files_file_list(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readSequence({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun fileRead(component: String?, apkPath: String): ByteArray? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_apkPath_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(apkPath))
    val __boltffi_apkPath_writer = __boltffi_apkPath_wire.writer
    __boltffi_apkPath_writer.writeString(apkPath)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_files_file_read(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_apkPath_wire.directBuffer(), __boltffi_apkPath_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readBytes() })
    } finally {
        __boltffi_component_wire.close()
        __boltffi_apkPath_wire.close()
    }
}

/**
 * The original, unmodified bytes of the component's APK file.
 */
internal fun fileSource(component: String?): ByteArray? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_files_file_source(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readBytes() })
    } finally {
        __boltffi_component_wire.close()
    }
}

/**
 * The DER-encoded X.509 certificate of each signer of the component's APK.
 */
internal fun fileSigners(component: String?): List<ByteArray> {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_files_file_signers(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readSequence({ __boltffi_reader -> __boltffi_reader.readBytes() })
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun fileInject(component: String?, apkPath: String, `data`: ByteArray, stored: Boolean) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_apkPath_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(apkPath))
    val __boltffi_apkPath_writer = __boltffi_apkPath_wire.writer
    __boltffi_apkPath_writer.writeString(apkPath)
    val __boltffi_data_wire = WireWriterPool.acquire(4 + `data`.size)
    val __boltffi_data_writer = __boltffi_data_wire.writer
    __boltffi_data_writer.writeBytes(`data`)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_files_file_inject(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_apkPath_wire.directBuffer(), __boltffi_apkPath_wire.size(), __boltffi_data_wire.directBuffer(), __boltffi_data_wire.size(), stored)
    } finally {
        __boltffi_component_wire.close()
        __boltffi_apkPath_wire.close()
        __boltffi_data_wire.close()
    }
}

internal fun fileDelete(component: String?, apkPath: String) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_apkPath_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(apkPath))
    val __boltffi_apkPath_writer = __boltffi_apkPath_wire.writer
    __boltffi_apkPath_writer.writeString(apkPath)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_files_file_delete(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_apkPath_wire.directBuffer(), __boltffi_apkPath_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_apkPath_wire.close()
    }
}

/**
 * Copies a file from the bundle into the APK.
 */
internal fun fileCopy(component: String?, bundleRelative: String, apkPath: String) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_bundleRelative_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(bundleRelative))
    val __boltffi_bundleRelative_writer = __boltffi_bundleRelative_wire.writer
    __boltffi_bundleRelative_writer.writeString(bundleRelative)
    val __boltffi_apkPath_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(apkPath))
    val __boltffi_apkPath_writer = __boltffi_apkPath_wire.writer
    __boltffi_apkPath_writer.writeString(apkPath)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_files_file_copy(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_bundleRelative_wire.directBuffer(), __boltffi_bundleRelative_wire.size(), __boltffi_apkPath_wire.directBuffer(), __boltffi_apkPath_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_bundleRelative_wire.close()
        __boltffi_apkPath_wire.close()
    }
}

internal fun logInfo(msg: String) {
    val __boltffi_msg_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(msg))
    val __boltffi_msg_writer = __boltffi_msg_wire.writer
    __boltffi_msg_writer.writeString(msg)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_log_host_log_info(__boltffi_msg_wire.directBuffer(), __boltffi_msg_wire.size())
    } finally {
        __boltffi_msg_wire.close()
    }
}

internal fun logWarn(msg: String) {
    val __boltffi_msg_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(msg))
    val __boltffi_msg_writer = __boltffi_msg_wire.writer
    __boltffi_msg_writer.writeString(msg)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_log_host_log_warn(__boltffi_msg_wire.directBuffer(), __boltffi_msg_wire.size())
    } finally {
        __boltffi_msg_wire.close()
    }
}

internal fun logDebug(msg: String) {
    val __boltffi_msg_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(msg))
    val __boltffi_msg_writer = __boltffi_msg_wire.writer
    __boltffi_msg_writer.writeString(msg)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_log_host_log_debug(__boltffi_msg_wire.directBuffer(), __boltffi_msg_wire.size())
    } finally {
        __boltffi_msg_wire.close()
    }
}

internal fun manifestPackageName(component: String?): String? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_package_name(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun manifestVersionCode(component: String?): UInt? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_version_code(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun manifestVersionName(component: String?): String? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_version_name(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun manifestMinSdkVersion(component: String?): UInt? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_min_sdk_version(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun manifestSplitName(component: String?): String? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_split_name(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun manifestSetVersionCode(component: String?, code: UInt) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_version_code(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), code.toInt())
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun manifestSetVersionName(component: String?, name: String) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_version_name(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_name_wire.close()
    }
}

internal fun manifestSetMinSdk(component: String?, sdk: UInt) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_min_sdk(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), sdk.toInt())
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun manifestAddPermission(component: String?, permission: String) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_permission_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(permission))
    val __boltffi_permission_writer = __boltffi_permission_wire.writer
    __boltffi_permission_writer.writeString(permission)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_add_permission(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_permission_wire.directBuffer(), __boltffi_permission_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_permission_wire.close()
    }
}

/**
 * Sets the `android:` attribute `attr_name` on the first element named
 * `element_name`, adding it when the element lacks it.
 */
internal fun manifestSetAttributeInt(component: String?, elementName: String, attrName: String, value: Int) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_elementName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(elementName))
    val __boltffi_elementName_writer = __boltffi_elementName_wire.writer
    __boltffi_elementName_writer.writeString(elementName)
    val __boltffi_attrName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(attrName))
    val __boltffi_attrName_writer = __boltffi_attrName_wire.writer
    __boltffi_attrName_writer.writeString(attrName)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_attribute_int(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_elementName_wire.directBuffer(), __boltffi_elementName_wire.size(), __boltffi_attrName_wire.directBuffer(), __boltffi_attrName_wire.size(), value)
    } finally {
        __boltffi_component_wire.close()
        __boltffi_elementName_wire.close()
        __boltffi_attrName_wire.close()
    }
}

internal fun manifestSetAttributeString(component: String?, elementName: String, attrName: String, value: String) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_elementName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(elementName))
    val __boltffi_elementName_writer = __boltffi_elementName_wire.writer
    __boltffi_elementName_writer.writeString(elementName)
    val __boltffi_attrName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(attrName))
    val __boltffi_attrName_writer = __boltffi_attrName_wire.writer
    __boltffi_attrName_writer.writeString(attrName)
    val __boltffi_value_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(value))
    val __boltffi_value_writer = __boltffi_value_wire.writer
    __boltffi_value_writer.writeString(value)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_attribute_string(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_elementName_wire.directBuffer(), __boltffi_elementName_wire.size(), __boltffi_attrName_wire.directBuffer(), __boltffi_attrName_wire.size(), __boltffi_value_wire.directBuffer(), __boltffi_value_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_elementName_wire.close()
        __boltffi_attrName_wire.close()
        __boltffi_value_wire.close()
    }
}

internal fun manifestSetActivityConfigChanges(component: String?, activityName: String, configChanges: String) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_activityName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(activityName))
    val __boltffi_activityName_writer = __boltffi_activityName_wire.writer
    __boltffi_activityName_writer.writeString(activityName)
    val __boltffi_configChanges_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(configChanges))
    val __boltffi_configChanges_writer = __boltffi_configChanges_wire.writer
    __boltffi_configChanges_writer.writeString(configChanges)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_set_activity_config_changes(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_activityName_wire.directBuffer(), __boltffi_activityName_wire.size(), __boltffi_configChanges_wire.directBuffer(), __boltffi_configChanges_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_activityName_wire.close()
        __boltffi_configChanges_wire.close()
    }
}

internal fun manifestAddIntentFilter(component: String?, activityName: String, action: String?, category: String?, mimeType: String?) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_activityName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(activityName))
    val __boltffi_activityName_writer = __boltffi_activityName_wire.writer
    __boltffi_activityName_writer.writeString(activityName)
    val __boltffi_action_wire = WireWriterPool.acquire(1 + (action?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_action_writer = __boltffi_action_wire.writer
    __boltffi_action_writer.writeOptionalValue(action, { __boltffi_action_writer, __boltffi_value_0 -> __boltffi_action_writer.writeString(__boltffi_value_0) })
    val __boltffi_category_wire = WireWriterPool.acquire(1 + (category?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_category_writer = __boltffi_category_wire.writer
    __boltffi_category_writer.writeOptionalValue(category, { __boltffi_category_writer, __boltffi_value_0 -> __boltffi_category_writer.writeString(__boltffi_value_0) })
    val __boltffi_mimeType_wire = WireWriterPool.acquire(1 + (mimeType?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_mimeType_writer = __boltffi_mimeType_wire.writer
    __boltffi_mimeType_writer.writeOptionalValue(mimeType, { __boltffi_mimeType_writer, __boltffi_value_0 -> __boltffi_mimeType_writer.writeString(__boltffi_value_0) })
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_add_intent_filter(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_activityName_wire.directBuffer(), __boltffi_activityName_wire.size(), __boltffi_action_wire.directBuffer(), __boltffi_action_wire.size(), __boltffi_category_wire.directBuffer(), __boltffi_category_wire.size(), __boltffi_mimeType_wire.directBuffer(), __boltffi_mimeType_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_activityName_wire.close()
        __boltffi_action_wire.close()
        __boltffi_category_wire.close()
        __boltffi_mimeType_wire.close()
    }
}

internal fun manifestAddActivityAlias(component: String?, targetActivity: String, aliasName: String, enabled: Boolean, label: String?) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_targetActivity_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(targetActivity))
    val __boltffi_targetActivity_writer = __boltffi_targetActivity_wire.writer
    __boltffi_targetActivity_writer.writeString(targetActivity)
    val __boltffi_aliasName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(aliasName))
    val __boltffi_aliasName_writer = __boltffi_aliasName_wire.writer
    __boltffi_aliasName_writer.writeString(aliasName)
    val __boltffi_label_wire = WireWriterPool.acquire(1 + (label?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_label_writer = __boltffi_label_wire.writer
    __boltffi_label_writer.writeOptionalValue(label, { __boltffi_label_writer, __boltffi_value_0 -> __boltffi_label_writer.writeString(__boltffi_value_0) })
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_add_activity_alias(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_targetActivity_wire.directBuffer(), __boltffi_targetActivity_wire.size(), __boltffi_aliasName_wire.directBuffer(), __boltffi_aliasName_wire.size(), enabled, __boltffi_label_wire.directBuffer(), __boltffi_label_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_targetActivity_wire.close()
        __boltffi_aliasName_wire.close()
        __boltffi_label_wire.close()
    }
}

/**
 * Copies every `intent-filter` of `from_activity` to the start of `to_activity`.
 */
internal fun manifestCopyIntentFilters(component: String?, fromActivity: String, toActivity: String) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_fromActivity_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(fromActivity))
    val __boltffi_fromActivity_writer = __boltffi_fromActivity_wire.writer
    __boltffi_fromActivity_writer.writeString(fromActivity)
    val __boltffi_toActivity_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(toActivity))
    val __boltffi_toActivity_writer = __boltffi_toActivity_wire.writer
    __boltffi_toActivity_writer.writeString(toActivity)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_copy_intent_filters(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_fromActivity_wire.directBuffer(), __boltffi_fromActivity_wire.size(), __boltffi_toActivity_wire.directBuffer(), __boltffi_toActivity_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_fromActivity_wire.close()
        __boltffi_toActivity_wire.close()
    }
}

/**
 * Opens the manifest as an XML document; edits through either view are
 * shared until the document is closed.
 */
internal fun manifestGetDocument(component: String?): UInt? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_manifest_manifest_get_document(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun optionGetString(key: String): String? {
    val __boltffi_key_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(key))
    val __boltffi_key_writer = __boltffi_key_wire.writer
    __boltffi_key_writer.writeString(key)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_options_option_get_string(__boltffi_key_wire.directBuffer(), __boltffi_key_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_key_wire.close()
    }
}

internal fun optionGetBool(key: String): Boolean? {
    val __boltffi_key_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(key))
    val __boltffi_key_writer = __boltffi_key_wire.writer
    __boltffi_key_writer.writeString(key)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_options_option_get_bool(__boltffi_key_wire.directBuffer(), __boltffi_key_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalBool()
    } finally {
        __boltffi_key_wire.close()
    }
}

internal fun optionGetInt(key: String): Long? {
    val __boltffi_key_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(key))
    val __boltffi_key_writer = __boltffi_key_wire.writer
    __boltffi_key_writer.writeString(key)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_options_option_get_int(__boltffi_key_wire.directBuffer(), __boltffi_key_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalI64()
    } finally {
        __boltffi_key_wire.close()
    }
}

internal fun optionGetFloat(key: String): Double? {
    val __boltffi_key_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(key))
    val __boltffi_key_writer = __boltffi_key_wire.writer
    __boltffi_key_writer.writeString(key)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_options_option_get_float(__boltffi_key_wire.directBuffer(), __boltffi_key_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalF64()
    } finally {
        __boltffi_key_wire.close()
    }
}

internal fun optionGetStringList(key: String): List<String>? {
    val __boltffi_key_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(key))
    val __boltffi_key_writer = __boltffi_key_wire.writer
    __boltffi_key_writer.writeString(key)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_options_option_get_string_list(__boltffi_key_wire.directBuffer(), __boltffi_key_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readSequence({ __boltffi_reader -> __boltffi_reader.readString() }) })
    } finally {
        __boltffi_key_wire.close()
    }
}

internal fun optionGetPath(key: String): String? {
    val __boltffi_key_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(key))
    val __boltffi_key_writer = __boltffi_key_wire.writer
    __boltffi_key_writer.writeString(key)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_options_option_get_path(__boltffi_key_wire.directBuffer(), __boltffi_key_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_key_wire.close()
    }
}

internal fun optionListPathContents(key: String): List<String>? {
    val __boltffi_key_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(key))
    val __boltffi_key_writer = __boltffi_key_wire.writer
    __boltffi_key_writer.writeString(key)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_options_option_list_path_contents(__boltffi_key_wire.directBuffer(), __boltffi_key_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readSequence({ __boltffi_reader -> __boltffi_reader.readString() }) })
    } finally {
        __boltffi_key_wire.close()
    }
}

internal fun optionReadPathFile(key: String, relativePath: String): ByteArray? {
    val __boltffi_key_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(key))
    val __boltffi_key_writer = __boltffi_key_wire.writer
    __boltffi_key_writer.writeString(key)
    val __boltffi_relativePath_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(relativePath))
    val __boltffi_relativePath_writer = __boltffi_relativePath_wire.writer
    __boltffi_relativePath_writer.writeString(relativePath)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_options_option_read_path_file(__boltffi_key_wire.directBuffer(), __boltffi_key_wire.size(), __boltffi_relativePath_wire.directBuffer(), __boltffi_relativePath_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readBytes() })
    } finally {
        __boltffi_key_wire.close()
        __boltffi_relativePath_wire.close()
    }
}

internal fun resComponentNames(): List<String> {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_component_names() ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readSequence({ __boltffi_reader -> __boltffi_reader.readString() })
}

/**
 * The component defining `res_type/res_name`, searching all of them.
 */
internal fun resComponentFor(resType: String, resName: String): String? {
    val __boltffi_resType_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resType))
    val __boltffi_resType_writer = __boltffi_resType_wire.writer
    __boltffi_resType_writer.writeString(resType)
    val __boltffi_resName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resName))
    val __boltffi_resName_writer = __boltffi_resName_wire.writer
    __boltffi_resName_writer.writeString(resName)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_component_for(__boltffi_resType_wire.directBuffer(), __boltffi_resType_wire.size(), __boltffi_resName_wire.directBuffer(), __boltffi_resName_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_resType_wire.close()
        __boltffi_resName_wire.close()
    }
}

internal fun resComponentForId(resId: UInt): String? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_component_for_id(resId.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
}

/**
 * The id of `res_type/res_name`; without a component every one is searched.
 */
internal fun resId(component: String?, resType: String, resName: String): UInt? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_resType_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resType))
    val __boltffi_resType_writer = __boltffi_resType_wire.writer
    __boltffi_resType_writer.writeString(resType)
    val __boltffi_resName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resName))
    val __boltffi_resName_writer = __boltffi_resName_wire.writer
    __boltffi_resName_writer.writeString(resName)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_id(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_resType_wire.directBuffer(), __boltffi_resType_wire.size(), __boltffi_resName_wire.directBuffer(), __boltffi_resName_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_component_wire.close()
        __boltffi_resType_wire.close()
        __boltffi_resName_wire.close()
    }
}

internal fun resExists(component: String?, resType: String, resName: String): Boolean {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_resType_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resType))
    val __boltffi_resType_writer = __boltffi_resType_wire.writer
    __boltffi_resType_writer.writeString(resType)
    val __boltffi_resName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resName))
    val __boltffi_resName_writer = __boltffi_resName_wire.writer
    __boltffi_resName_writer.writeString(resName)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_resources_res_exists(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_resType_wire.directBuffer(), __boltffi_resType_wire.size(), __boltffi_resName_wire.directBuffer(), __boltffi_resName_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_resType_wire.close()
        __boltffi_resName_wire.close()
    }
}

internal fun resGetString(component: String?, name: String): String? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_get_string(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_component_wire.close()
        __boltffi_name_wire.close()
    }
}

internal fun resSetString(component: String?, name: String, value: String): Boolean {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    val __boltffi_value_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(value))
    val __boltffi_value_writer = __boltffi_value_wire.writer
    __boltffi_value_writer.writeString(value)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_resources_res_set_string(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size(), __boltffi_value_wire.directBuffer(), __boltffi_value_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_name_wire.close()
        __boltffi_value_wire.close()
    }
}

/**
 * Adds `res_type/name` with `value` read the way resource XML is: booleans,
 * integers, colors, dimensions and `@type/name` references. A `string`
 * entry keeps the text as is.
 */
internal fun resAdd(component: String?, resType: String, name: String, value: String): UInt? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_resType_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resType))
    val __boltffi_resType_writer = __boltffi_resType_wire.writer
    __boltffi_resType_writer.writeString(resType)
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    val __boltffi_value_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(value))
    val __boltffi_value_writer = __boltffi_value_wire.writer
    __boltffi_value_writer.writeString(value)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_add(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_resType_wire.directBuffer(), __boltffi_resType_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size(), __boltffi_value_wire.directBuffer(), __boltffi_value_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_component_wire.close()
        __boltffi_resType_wire.close()
        __boltffi_name_wire.close()
        __boltffi_value_wire.close()
    }
}

internal fun resAddId(component: String?, name: String): UInt? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_add_id(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_component_wire.close()
        __boltffi_name_wire.close()
    }
}

internal fun resAddRaw(component: String?, resType: String, name: String, dataType: UByte, `data`: UInt): UInt? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_resType_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resType))
    val __boltffi_resType_writer = __boltffi_resType_wire.writer
    __boltffi_resType_writer.writeString(resType)
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_add_raw(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_resType_wire.directBuffer(), __boltffi_resType_wire.size(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size(), dataType.toByte(), `data`.toInt()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_component_wire.close()
        __boltffi_resType_wire.close()
        __boltffi_name_wire.close()
    }
}

internal fun resGetRaw(component: String?, resType: String, resName: String): Long? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_resType_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resType))
    val __boltffi_resType_writer = __boltffi_resType_wire.writer
    __boltffi_resType_writer.writeString(resType)
    val __boltffi_resName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resName))
    val __boltffi_resName_writer = __boltffi_resName_wire.writer
    __boltffi_resName_writer.writeString(resName)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_get_raw(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_resType_wire.directBuffer(), __boltffi_resType_wire.size(), __boltffi_resName_wire.directBuffer(), __boltffi_resName_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalI64()
    } finally {
        __boltffi_component_wire.close()
        __boltffi_resType_wire.close()
        __boltffi_resName_wire.close()
    }
}

internal fun resCopy(bundleRelative: String, apkPath: String) {
    val __boltffi_bundleRelative_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(bundleRelative))
    val __boltffi_bundleRelative_writer = __boltffi_bundleRelative_wire.writer
    __boltffi_bundleRelative_writer.writeString(bundleRelative)
    val __boltffi_apkPath_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(apkPath))
    val __boltffi_apkPath_writer = __boltffi_apkPath_wire.writer
    __boltffi_apkPath_writer.writeString(apkPath)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_resources_res_copy(__boltffi_bundleRelative_wire.directBuffer(), __boltffi_bundleRelative_wire.size(), __boltffi_apkPath_wire.directBuffer(), __boltffi_apkPath_wire.size())
    } finally {
        __boltffi_bundleRelative_wire.close()
        __boltffi_apkPath_wire.close()
    }
}

/**
 * Copies `resources/<res_type>/<file>` from the bundle into `res/<res_type>/`.
 */
internal fun resCopyGroup(resType: String, files: List<String>) {
    val __boltffi_resType_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(resType))
    val __boltffi_resType_writer = __boltffi_resType_wire.writer
    __boltffi_resType_writer.writeString(resType)
    val __boltffi_files_wire = WireWriterPool.acquire(4 + files.sumOf { __boltffi_value_0 -> (4 + Utf8Codec.maxBytes(__boltffi_value_0)).toInt() })
    val __boltffi_files_writer = __boltffi_files_wire.writer
    __boltffi_files_writer.writeSequence(files, files.size, { __boltffi_files_writer, __boltffi_value_0 -> __boltffi_files_writer.writeString(__boltffi_value_0) })
    try {
        Native.boltffi_function_reseam_patcher_kotlin_resources_res_copy_group(__boltffi_resType_wire.directBuffer(), __boltffi_resType_wire.size(), __boltffi_files_wire.directBuffer(), __boltffi_files_wire.size())
    } finally {
        __boltffi_resType_wire.close()
        __boltffi_files_wire.close()
    }
}

internal fun resInject(apkPath: String, `data`: ByteArray) {
    val __boltffi_apkPath_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(apkPath))
    val __boltffi_apkPath_writer = __boltffi_apkPath_wire.writer
    __boltffi_apkPath_writer.writeString(apkPath)
    val __boltffi_data_wire = WireWriterPool.acquire(4 + `data`.size)
    val __boltffi_data_writer = __boltffi_data_wire.writer
    __boltffi_data_writer.writeBytes(`data`)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_resources_res_inject(__boltffi_apkPath_wire.directBuffer(), __boltffi_apkPath_wire.size(), __boltffi_data_wire.directBuffer(), __boltffi_data_wire.size())
    } finally {
        __boltffi_apkPath_wire.close()
        __boltffi_data_wire.close()
    }
}

internal fun resDelete(apkPath: String) {
    val __boltffi_apkPath_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(apkPath))
    val __boltffi_apkPath_writer = __boltffi_apkPath_wire.writer
    __boltffi_apkPath_writer.writeString(apkPath)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_resources_res_delete(__boltffi_apkPath_wire.directBuffer(), __boltffi_apkPath_wire.size())
    } finally {
        __boltffi_apkPath_wire.close()
    }
}

internal fun resList(prefix: String): List<String> {
    val __boltffi_prefix_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(prefix))
    val __boltffi_prefix_writer = __boltffi_prefix_wire.writer
    __boltffi_prefix_writer.writeString(prefix)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_list(__boltffi_prefix_wire.directBuffer(), __boltffi_prefix_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readSequence({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_prefix_wire.close()
    }
}

internal fun resPoolGet(component: String?, index: UInt): String? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_pool_get(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), index.toInt()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_component_wire.close()
    }
}

internal fun resPoolSet(component: String?, index: UInt, value: String) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_value_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(value))
    val __boltffi_value_writer = __boltffi_value_wire.writer
    __boltffi_value_writer.writeString(value)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_resources_res_pool_set(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), index.toInt(), __boltffi_value_wire.directBuffer(), __boltffi_value_wire.size())
    } finally {
        __boltffi_component_wire.close()
        __boltffi_value_wire.close()
    }
}

internal fun resPoolAdd(component: String?, value: String): UInt? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_value_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(value))
    val __boltffi_value_writer = __boltffi_value_wire.writer
    __boltffi_value_writer.writeString(value)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_pool_add(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_value_wire.directBuffer(), __boltffi_value_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_component_wire.close()
        __boltffi_value_wire.close()
    }
}

internal fun resPoolFindRefs(component: String?, stringIndex: UInt): List<ResourceRef> {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_resources_res_pool_find_refs(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), stringIndex.toInt()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readSequence({ __boltffi_reader -> ResourceRef.fromReader(__boltffi_reader) })
    } finally {
        __boltffi_component_wire.close()
    }
}

/**
 * Points a string entry at another pool string; without a component the
 * entry's own component is used.
 */
internal fun resReplaceEntry(component: String?, resId: UInt, newStringIndex: UInt) {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    try {
        Native.boltffi_function_reseam_patcher_kotlin_resources_res_replace_entry(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), resId.toInt(), newStringIndex.toInt())
    } finally {
        __boltffi_component_wire.close()
    }
}

/**
 * Opens `apk_path` from the component (base when `None`) as a document.
 */
internal fun xmlOpen(component: String?, apkPath: String): UInt? {
    val __boltffi_component_wire = WireWriterPool.acquire(1 + (component?.let { __boltffi_value_0 -> 4 + Utf8Codec.maxBytes(__boltffi_value_0) } ?: 0))
    val __boltffi_component_writer = __boltffi_component_wire.writer
    __boltffi_component_writer.writeOptionalValue(component, { __boltffi_component_writer, __boltffi_value_0 -> __boltffi_component_writer.writeString(__boltffi_value_0) })
    val __boltffi_apkPath_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(apkPath))
    val __boltffi_apkPath_writer = __boltffi_apkPath_wire.writer
    __boltffi_apkPath_writer.writeString(apkPath)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_xml_xml_open(__boltffi_component_wire.directBuffer(), __boltffi_component_wire.size(), __boltffi_apkPath_wire.directBuffer(), __boltffi_apkPath_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalU32()
    } finally {
        __boltffi_component_wire.close()
        __boltffi_apkPath_wire.close()
    }
}

/**
 * Writes the document back to the APK and releases its handle.
 */
internal fun xmlClose(doc: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_xml_xml_close(doc.toInt())
}

internal fun xmlRoot(doc: UInt): UInt {
    return Native.boltffi_function_reseam_patcher_kotlin_xml_xml_root(doc.toInt()).toUInt()
}

internal fun xmlFindByTag(doc: UInt, tag: String): UIntArray {
    val __boltffi_tag_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(tag))
    val __boltffi_tag_writer = __boltffi_tag_wire.writer
    __boltffi_tag_writer.writeString(tag)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_xml_xml_find_by_tag(doc.toInt(), __boltffi_tag_wire.directBuffer(), __boltffi_tag_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readUIntArray(__boltffi_result)
    } finally {
        __boltffi_tag_wire.close()
    }
}

internal fun xmlFindByAttribute(doc: UInt, attrName: String, attrValue: String): UIntArray {
    val __boltffi_attrName_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(attrName))
    val __boltffi_attrName_writer = __boltffi_attrName_wire.writer
    __boltffi_attrName_writer.writeString(attrName)
    val __boltffi_attrValue_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(attrValue))
    val __boltffi_attrValue_writer = __boltffi_attrValue_wire.writer
    __boltffi_attrValue_writer.writeString(attrValue)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_xml_xml_find_by_attribute(doc.toInt(), __boltffi_attrName_wire.directBuffer(), __boltffi_attrName_wire.size(), __boltffi_attrValue_wire.directBuffer(), __boltffi_attrValue_wire.size()) ?: throw IllegalStateException("null buffer returned")
        return DirectVectorCodec.readUIntArray(__boltffi_result)
    } finally {
        __boltffi_attrName_wire.close()
        __boltffi_attrValue_wire.close()
    }
}

internal fun xmlChildren(doc: UInt, el: UInt): UIntArray {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_xml_xml_children(doc.toInt(), el.toInt()) ?: throw IllegalStateException("null buffer returned")
    return DirectVectorCodec.readUIntArray(__boltffi_result)
}

internal fun xmlParent(doc: UInt, el: UInt): UInt? {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_xml_xml_parent(doc.toInt(), el.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readOptionalU32()
}

internal fun xmlTagName(doc: UInt, el: UInt): String {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_xml_xml_tag_name(doc.toInt(), el.toInt()) ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readString()
}

internal fun xmlGetAttribute(doc: UInt, el: UInt, name: String): String? {
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_xml_xml_get_attribute(doc.toInt(), el.toInt(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size()) ?: throw IllegalStateException("null buffer returned")
        val __boltffi_reader = WireReader(__boltffi_result)
        return __boltffi_reader.readOptionalValue({ __boltffi_reader -> __boltffi_reader.readString() })
    } finally {
        __boltffi_name_wire.close()
    }
}

/**
 * Sets an attribute from text, parsing literals and resource references the
 * way the XML compiler does.
 */
internal fun xmlSetAttribute(doc: UInt, el: UInt, name: String, value: String) {
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    val __boltffi_value_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(value))
    val __boltffi_value_writer = __boltffi_value_wire.writer
    __boltffi_value_writer.writeString(value)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_xml_xml_set_attribute(doc.toInt(), el.toInt(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size(), __boltffi_value_wire.directBuffer(), __boltffi_value_wire.size())
    } finally {
        __boltffi_name_wire.close()
        __boltffi_value_wire.close()
    }
}

internal fun xmlSetAttributeRef(doc: UInt, el: UInt, name: String, resId: UInt) {
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_xml_xml_set_attribute_ref(doc.toInt(), el.toInt(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size(), resId.toInt())
    } finally {
        __boltffi_name_wire.close()
    }
}

internal fun xmlRemoveAttribute(doc: UInt, el: UInt, name: String) {
    val __boltffi_name_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(name))
    val __boltffi_name_writer = __boltffi_name_wire.writer
    __boltffi_name_writer.writeString(name)
    try {
        Native.boltffi_function_reseam_patcher_kotlin_xml_xml_remove_attribute(doc.toInt(), el.toInt(), __boltffi_name_wire.directBuffer(), __boltffi_name_wire.size())
    } finally {
        __boltffi_name_wire.close()
    }
}

/**
 * A detached element; attach it with `xml_append_child` or `xml_insert_before`.
 */
internal fun xmlCreateElement(doc: UInt, tag: String): UInt {
    val __boltffi_tag_wire = WireWriterPool.acquire(4 + Utf8Codec.maxBytes(tag))
    val __boltffi_tag_writer = __boltffi_tag_wire.writer
    __boltffi_tag_writer.writeString(tag)
    try {
        return Native.boltffi_function_reseam_patcher_kotlin_xml_xml_create_element(doc.toInt(), __boltffi_tag_wire.directBuffer(), __boltffi_tag_wire.size()).toUInt()
    } finally {
        __boltffi_tag_wire.close()
    }
}

internal fun xmlAppendChild(doc: UInt, parent: UInt, child: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_xml_xml_append_child(doc.toInt(), parent.toInt(), child.toInt())
}

internal fun xmlInsertBefore(doc: UInt, child: UInt, before: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_xml_xml_insert_before(doc.toInt(), child.toInt(), before.toInt())
}

internal fun xmlRemoveElement(doc: UInt, el: UInt) {
    Native.boltffi_function_reseam_patcher_kotlin_xml_xml_remove_element(doc.toInt(), el.toInt())
}

/**
 * A detached copy of an element, with or without its children.
 */
internal fun xmlCloneElement(doc: UInt, el: UInt, deep: Boolean): UInt {
    return Native.boltffi_function_reseam_patcher_kotlin_xml_xml_clone_element(doc.toInt(), el.toInt(), deep).toUInt()
}

internal fun ctxIsActive(): Boolean {
    return Native.boltffi_function_reseam_patcher_kotlin_ctx_is_active()
}

internal fun version(): String {
    val __boltffi_result = Native.boltffi_function_reseam_patcher_kotlin_version() ?: throw IllegalStateException("null buffer returned")
    val __boltffi_reader = WireReader(__boltffi_result)
    return __boltffi_reader.readString()
}
