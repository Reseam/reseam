package dev.stitch.patch

import kotlin.properties.ReadOnlyProperty
import kotlin.reflect.KProperty

internal class StringOption(
    private val key: String,
    private val default: String? = null,
) : ReadOnlyProperty<Any?, String?> {
    override fun getValue(thisRef: Any?, property: KProperty<*>): String? =
        optionGetString(key) ?: default
}

internal class BoolOption(
    private val key: String,
    private val default: Boolean? = null,
) : ReadOnlyProperty<Any?, Boolean?> {
    override fun getValue(thisRef: Any?, property: KProperty<*>): Boolean? =
        optionGetBool(key) ?: default
}

internal class IntOption(
    private val key: String,
    private val default: Long? = null,
) : ReadOnlyProperty<Any?, Long?> {
    override fun getValue(thisRef: Any?, property: KProperty<*>): Long? =
        optionGetInt(key) ?: default
}

internal class FloatOption(
    private val key: String,
    private val default: Double? = null,
) : ReadOnlyProperty<Any?, Double?> {
    override fun getValue(thisRef: Any?, property: KProperty<*>): Double? =
        optionGetFloat(key) ?: default
}

internal class StringListOption(
    private val key: String,
    private val default: List<String>? = null,
) : ReadOnlyProperty<Any?, List<String>?> {
    override fun getValue(thisRef: Any?, property: KProperty<*>): List<String>? =
        optionGetStringList(key) ?: default
}

fun stringOption(key: String, default: String? = null): ReadOnlyProperty<Any?, String?> = StringOption(key, default)
fun boolOption(key: String, default: Boolean? = null): ReadOnlyProperty<Any?, Boolean?> = BoolOption(key, default)
fun intOption(key: String, default: Long? = null): ReadOnlyProperty<Any?, Long?> = IntOption(key, default)
fun floatOption(key: String, default: Double? = null): ReadOnlyProperty<Any?, Double?> = FloatOption(key, default)
fun stringListOption(key: String, default: List<String>? = null): ReadOnlyProperty<Any?, List<String>?> = StringListOption(key, default)
