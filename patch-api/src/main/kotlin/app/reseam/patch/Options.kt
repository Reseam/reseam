// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.native.optionGetBool
import app.reseam.patch.native.optionGetFloat
import app.reseam.patch.native.optionGetInt
import app.reseam.patch.native.optionGetPath
import app.reseam.patch.native.optionGetString
import app.reseam.patch.native.optionGetStringList
import app.reseam.patch.native.optionListPathContents
import app.reseam.patch.native.optionReadPathFile
import app.reseam.patch.settings.StringRef

enum class OptionKind {
    STRING,
    BOOL,
    INT,
    FLOAT,
    STRING_LIST,
    PATH,
}

/**
 * A value the user supplies when applying the patch; declared in the patch block, read with
 * `options[it]`.
 */
sealed class Option<T : Any>(
    val key: String,
    val title: StringRef,
    val description: StringRef?,
    val required: Boolean,
    val kind: OptionKind,
    val default: T?,
    val validValues: List<String>?,
) {
    /** Values users pick by label. Without [allowsCustom], the only ones accepted. */
    open val presets: List<OptionPreset<T>>
        get() = emptyList()

    open val allowsCustom: Boolean
        get() = true

    internal abstract fun read(): T?

    override fun toString() = key
}

/** A value an option offers under [label], which users pick it by. */
class OptionPreset<T : Any>(val label: StringRef, val value: T)

class StringOption
internal constructor(
    key: String,
    title: StringRef,
    description: StringRef?,
    required: Boolean,
    default: String?,
    validValues: List<String>?,
    override val presets: List<OptionPreset<String>>,
    override val allowsCustom: Boolean,
) :
    Option<String>(
        key,
        title,
        description,
        required,
        OptionKind.STRING,
        default,
        validValues,
    ) {
    override fun read() = optionGetString(key)
}

class BoolOption
internal constructor(
    key: String,
    title: StringRef,
    description: StringRef?,
    required: Boolean,
    default: Boolean?,
) : Option<Boolean>(key, title, description, required, OptionKind.BOOL, default, null) {
    override fun read() = optionGetBool(key)
}

class IntOption
internal constructor(
    key: String,
    title: StringRef,
    description: StringRef?,
    required: Boolean,
    default: Long?,
    override val presets: List<OptionPreset<Long>>,
    override val allowsCustom: Boolean,
) :
    Option<Long>(
        key,
        title,
        description,
        required,
        OptionKind.INT,
        default,
        null,
    ) {
    override fun read() = optionGetInt(key)
}

class FloatOption
internal constructor(
    key: String,
    title: StringRef,
    description: StringRef?,
    required: Boolean,
    default: Double?,
    override val presets: List<OptionPreset<Double>>,
    override val allowsCustom: Boolean,
) :
    Option<Double>(
        key,
        title,
        description,
        required,
        OptionKind.FLOAT,
        default,
        null,
    ) {
    override fun read() = optionGetFloat(key)
}

class StringListOption
internal constructor(
    key: String,
    title: StringRef,
    description: StringRef?,
    required: Boolean,
    default: List<String>?,
    override val presets: List<OptionPreset<List<String>>>,
    override val allowsCustom: Boolean,
) :
    Option<List<String>>(
        key,
        title,
        description,
        required,
        OptionKind.STRING_LIST,
        default,
        null,
    ) {
    override fun read() = optionGetStringList(key)
}

class PathOption
internal constructor(key: String, title: StringRef, description: StringRef?, required: Boolean) :
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
    /**
     * The option's value; the engine fills in defaults, so only an optional option without one is
     * absent.
     */
    operator fun <T : Any> get(option: Option<T>): T =
        option.read()
            ?: error(
                "Option '${option.key}' has no value. Declare a default, mark it required, or read it with getOrNull."
            )

    fun <T : Any> getOrNull(option: Option<T>): T? = option.read()
}

internal fun <T : Any> List<OptionPreset<T>>.checked(key: String, allowCustom: Boolean) = also {
    require(allowCustom || isNotEmpty()) {
        "Option '$key' allows no custom value, so it needs presets"
    }
}
