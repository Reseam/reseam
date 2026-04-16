// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

class PathOptionValue internal constructor(
    val path: String,
    private val key: String,
) {
    fun listContents(): List<String>? = optionListPathContents(key)
    fun readFile(relativePath: String): ByteArray? = optionReadPathFile(key, relativePath)
}

class RuntimeOptions internal constructor() {
    fun string(key: String): String? = optionGetString(key)
    fun bool(key: String): Boolean? = optionGetBool(key)
    fun int(key: String): Long? = optionGetInt(key)
    fun float(key: String): Double? = optionGetFloat(key)
    fun stringList(key: String): List<String>? = optionGetStringList(key)
    fun path(key: String): PathOptionValue? = optionGetPath(key)?.let { PathOptionValue(it, key) }
}

fun optionsOf(vararg declarations: PatchOption): List<PatchOption> = declarations.toList()

fun stringOption(
    key: String,
    default: String? = null,
    title: String? = null,
    description: String? = null,
    validValues: List<String>? = null,
    required: Boolean = false,
): PatchOption = PatchOption(
    key = key,
    title = title ?: key,
    description = description.orEmpty(),
    type = PatchOptionType.STRING,
    defaultString = default,
    validValues = validValues,
    required = required,
)

fun boolOption(
    key: String,
    default: Boolean? = null,
    title: String? = null,
    description: String? = null,
    required: Boolean = false,
): PatchOption = PatchOption(
    key = key,
    title = title ?: key,
    description = description.orEmpty(),
    type = PatchOptionType.BOOL,
    defaultBool = default,
    required = required,
)

fun intOption(
    key: String,
    default: Long? = null,
    title: String? = null,
    description: String? = null,
    required: Boolean = false,
): PatchOption = PatchOption(
    key = key,
    title = title ?: key,
    description = description.orEmpty(),
    type = PatchOptionType.INT,
    defaultInt = default,
    required = required,
)

fun floatOption(
    key: String,
    default: Double? = null,
    title: String? = null,
    description: String? = null,
    required: Boolean = false,
): PatchOption = PatchOption(
    key = key,
    title = title ?: key,
    description = description.orEmpty(),
    type = PatchOptionType.FLOAT,
    defaultFloat = default,
    required = required,
)

fun stringListOption(
    key: String,
    default: List<String>? = null,
    title: String? = null,
    description: String? = null,
    required: Boolean = false,
): PatchOption = PatchOption(
    key = key,
    title = title ?: key,
    description = description.orEmpty(),
    type = PatchOptionType.STRING_LIST,
    defaultStringList = default,
    required = required,
)

fun pathOption(
    key: String,
    title: String? = null,
    description: String? = null,
    required: Boolean = false,
): PatchOption = PatchOption(
    key = key,
    title = title ?: key,
    description = description.orEmpty(),
    type = PatchOptionType.PATH,
    required = required,
)
