// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

enum class OptionKind { STRING, BOOL, INT, FLOAT, STRING_LIST, PATH }

/** A value the user supplies when applying the patch; declared in the patch block, read with `options[it]`. */
sealed class Option<T : Any>(
    val key: String,
    val title: String,
    val description: String,
    val required: Boolean,
    val kind: OptionKind,
    val default: T?,
    val validValues: List<String>?,
) {
    internal abstract fun read(): T?

    override fun toString() = key
}

class StringOption internal constructor(key: String, title: String, description: String, required: Boolean, default: String?, validValues: List<String>?) :
    Option<String>(key, title, description, required, OptionKind.STRING, default, validValues) {
    override fun read() = optionGetString(key)
}

class BoolOption internal constructor(key: String, title: String, description: String, required: Boolean, default: Boolean?) :
    Option<Boolean>(key, title, description, required, OptionKind.BOOL, default, null) {
    override fun read() = optionGetBool(key)
}

class IntOption internal constructor(key: String, title: String, description: String, required: Boolean, default: Long?) :
    Option<Long>(key, title, description, required, OptionKind.INT, default, null) {
    override fun read() = optionGetInt(key)
}

class FloatOption internal constructor(key: String, title: String, description: String, required: Boolean, default: Double?) :
    Option<Double>(key, title, description, required, OptionKind.FLOAT, default, null) {
    override fun read() = optionGetFloat(key)
}

class StringListOption internal constructor(key: String, title: String, description: String, required: Boolean, default: List<String>?) :
    Option<List<String>>(key, title, description, required, OptionKind.STRING_LIST, default, null) {
    override fun read() = optionGetStringList(key)
}

class PathOption internal constructor(key: String, title: String, description: String, required: Boolean) :
    Option<OptionPath>(key, title, description, required, OptionKind.PATH, null, null) {
    override fun read() = optionGetPath(key)?.let { OptionPath(it, key) }
}

/** A directory the user picked for a [PathOption]. */
class OptionPath internal constructor(val path: String, private val key: String) {
    fun listContents(): List<String> = optionListPathContents(key).orEmpty()
    fun readFile(relativePath: String): ByteArray? = optionReadPathFile(key, relativePath)
    override fun toString() = path
}

class RuntimeOptions internal constructor() {
    /** The option's value; the engine fills in defaults, so only an optional option without one is absent. */
    operator fun <T : Any> get(option: Option<T>): T =
        option.read() ?: error("Option '${option.key}' has no value. Declare a default, mark it required, or read it with getOrNull.")

    fun <T : Any> getOrNull(option: Option<T>): T? = option.read()
}
