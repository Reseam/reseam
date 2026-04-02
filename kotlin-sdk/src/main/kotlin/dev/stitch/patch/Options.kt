package dev.stitch.patch

import kotlin.properties.ReadOnlyProperty
import kotlin.reflect.KProperty

interface DeclaredOption<T> : ReadOnlyProperty<Any?, T?> {
    val declaration: PatchOption
}

private abstract class BaseOption<T>(
    private val key: String,
    final override val declaration: PatchOption,
) : DeclaredOption<T>

private class StringOption(
    key: String,
    private val default: String?,
    declaration: PatchOption,
) : BaseOption<String>(key, declaration) {
    private val optionKey = key

    override fun getValue(thisRef: Any?, property: KProperty<*>): String? =
        optionGetString(optionKey) ?: default
}

private class BoolOption(
    key: String,
    private val default: Boolean?,
    declaration: PatchOption,
) : BaseOption<Boolean>(key, declaration) {
    private val optionKey = key

    override fun getValue(thisRef: Any?, property: KProperty<*>): Boolean? =
        optionGetBool(optionKey) ?: default
}

private class IntOption(
    key: String,
    private val default: Long?,
    declaration: PatchOption,
) : BaseOption<Long>(key, declaration) {
    private val optionKey = key

    override fun getValue(thisRef: Any?, property: KProperty<*>): Long? =
        optionGetInt(optionKey) ?: default
}

private class FloatOption(
    key: String,
    private val default: Double?,
    declaration: PatchOption,
) : BaseOption<Double>(key, declaration) {
    private val optionKey = key

    override fun getValue(thisRef: Any?, property: KProperty<*>): Double? =
        optionGetFloat(optionKey) ?: default
}

private class StringListOption(
    key: String,
    private val default: List<String>?,
    declaration: PatchOption,
) : BaseOption<List<String>>(key, declaration) {
    private val optionKey = key

    override fun getValue(thisRef: Any?, property: KProperty<*>): List<String>? =
        optionGetStringList(optionKey) ?: default
}

fun optionsOf(vararg declarations: DeclaredOption<*>): List<PatchOption> =
    declarations.map { it.declaration }

// These helpers declare option metadata for the host and also provide delegates for
// reading the resolved execution-time values selected by the caller.

fun stringOption(
    key: String,
    default: String? = null,
    title: String? = null,
    description: String? = null,
    validValues: List<String>? = null,
    required: Boolean = false,
): DeclaredOption<String> = StringOption(
    key = key,
    default = default,
    declaration = PatchOption(
        key = key,
        title = title ?: key,
        description = description.orEmpty(),
        type = PatchOptionType.STRING,
        defaultString = default,
        validValues = validValues,
        required = required,
    ),
)

fun boolOption(
    key: String,
    default: Boolean? = null,
    title: String? = null,
    description: String? = null,
    required: Boolean = false,
): DeclaredOption<Boolean> = BoolOption(
    key = key,
    default = default,
    declaration = PatchOption(
        key = key,
        title = title ?: key,
        description = description.orEmpty(),
        type = PatchOptionType.BOOL,
        defaultBool = default,
        required = required,
    ),
)

fun intOption(
    key: String,
    default: Long? = null,
    title: String? = null,
    description: String? = null,
    required: Boolean = false,
): DeclaredOption<Long> = IntOption(
    key = key,
    default = default,
    declaration = PatchOption(
        key = key,
        title = title ?: key,
        description = description.orEmpty(),
        type = PatchOptionType.INT,
        defaultInt = default,
        required = required,
    ),
)

fun floatOption(
    key: String,
    default: Double? = null,
    title: String? = null,
    description: String? = null,
    required: Boolean = false,
): DeclaredOption<Double> = FloatOption(
    key = key,
    default = default,
    declaration = PatchOption(
        key = key,
        title = title ?: key,
        description = description.orEmpty(),
        type = PatchOptionType.FLOAT,
        defaultFloat = default,
        required = required,
    ),
)

fun stringListOption(
    key: String,
    default: List<String>? = null,
    title: String? = null,
    description: String? = null,
    required: Boolean = false,
): DeclaredOption<List<String>> = StringListOption(
    key = key,
    default = default,
    declaration = PatchOption(
        key = key,
        title = title ?: key,
        description = description.orEmpty(),
        type = PatchOptionType.STRING_LIST,
        defaultStringList = default,
        required = required,
    ),
)
