// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

class ResourceScope internal constructor(
    private val componentName: String? = null,
) {
    fun components(): List<String> = resComponentNames()

    fun component(name: String): ResourceScope = ResourceScope(name)

    fun split(name: String): ResourceScope = component(name)

    fun owningComponent(resType: String, resName: String): String? = resComponentFor(resType, resName)

    fun owningComponent(resId: UInt): String? = resComponentForId(resId)

    fun id(resType: String, resName: String): UInt? = resId(componentName, resType, resName)

    fun exists(resType: String, resName: String): Boolean = resExists(componentName, resType, resName)

    fun getString(name: String): String? = resGetString(componentName, name)

    fun setString(name: String, value: String): Boolean = resSetString(componentName, name, value)

    fun add(resType: String, name: String, value: String): UInt? = resAdd(componentName, resType, name, value)

    fun addString(name: String, value: String): UInt? = add("string", name, value)

    fun addBool(name: String, value: Boolean): UInt? = add("bool", name, value.toString())

    fun addInteger(name: String, value: Int): UInt? = add("integer", name, value.toString())

    fun addColor(name: String, color: String): UInt? = add("color", name, color)

    fun addDimen(name: String, dimen: String): UInt? = add("dimen", name, dimen)

    fun addId(name: String): UInt? = resAddId(componentName, name)

    fun addRaw(resType: String, name: String, dataType: UByte, data: UInt): UInt? =
        resAddRaw(componentName, resType, name, dataType, data)

    fun getRaw(resType: String, resName: String): Long? = resGetRaw(componentName, resType, resName)

    fun poolGet(index: UInt): String? = resPoolGet(componentName, index)

    fun poolSet(index: UInt, value: String) = resPoolSet(componentName, index, value)

    fun poolAdd(value: String): UInt? = resPoolAdd(componentName, value)

    fun poolFindRefs(stringIndex: UInt): List<ResourceRef> = resPoolFindRefs(componentName, stringIndex)

    fun replaceEntry(resId: UInt, newStringIndex: UInt) = resReplaceEntry(componentName, resId, newStringIndex)
}
