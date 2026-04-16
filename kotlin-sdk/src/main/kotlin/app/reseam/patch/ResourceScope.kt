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

    fun owningComponent(resType: String, resName: String): String? =
        resComponentFor(resType, resName)

    fun owningComponent(resId: UInt): String? =
        resComponentForId(resId)

    fun id(resType: String, resName: String): UInt? =
        componentName?.let { resIdInComponent(it, resType, resName) } ?: resId(resType, resName)

    fun exists(resType: String, resName: String): Boolean =
        componentName?.let { resExistsInComponent(it, resType, resName) } ?: resExists(resType, resName)

    fun getString(name: String): String? =
        componentName?.let { resGetStringInComponent(it, name) } ?: resGetString(name)

    fun setString(name: String, value: String): Boolean =
        componentName?.let { resSetStringInComponent(it, name, value) } ?: resSetString(name, value)

    fun addString(name: String, value: String): UInt? =
        componentName?.let { resAddStringInComponent(it, name, value) } ?: resAddString(name, value)

    fun addBool(name: String, value: Boolean): UInt? =
        componentName?.let { resAddBoolInComponent(it, name, value) } ?: resAddBool(name, value)

    fun addInteger(name: String, value: Int): UInt? =
        componentName?.let { resAddIntegerInComponent(it, name, value) } ?: resAddInteger(name, value)

    fun addColor(name: String, color: String): UInt? =
        componentName?.let { resAddColorInComponent(it, name, color) } ?: resAddColor(name, color)

    fun addDimen(name: String, dimen: String): UInt? =
        componentName?.let { resAddDimenInComponent(it, name, dimen) } ?: resAddDimen(name, dimen)

    fun addId(name: String): UInt? =
        componentName?.let { resAddIdInComponent(it, name) } ?: resAddId(name)

    fun addRaw(resType: String, name: String, dataType: UByte, data: UInt): UInt? =
        componentName?.let { resAddRawInComponent(it, resType, name, dataType, data) }
            ?: resAddRaw(resType, name, dataType, data)

    fun getRaw(resType: String, resName: String): Long? =
        componentName?.let { resGetRawInComponent(it, resType, resName) } ?: resGetRaw(resType, resName)

    fun poolGet(index: UInt): String? =
        componentName?.let { resPoolGetInComponent(it, index) } ?: resPoolGet(index)

    fun poolSet(index: UInt, value: String) {
        if (componentName == null) {
            resPoolSet(index, value)
        } else {
            resPoolSetInComponent(componentName, index, value)
        }
    }

    fun poolAdd(value: String): UInt? =
        componentName?.let { resPoolAddInComponent(it, value) } ?: resPoolAdd(value)

    fun poolFindRefs(stringIndex: UInt): List<ResourceRef> =
        componentName?.let { resPoolFindRefsInComponent(it, stringIndex) } ?: resPoolFindRefs(stringIndex)

    fun replaceEntry(resId: UInt, newStringIndex: UInt) {
        if (componentName == null) {
            resReplaceEntry(resId, newStringIndex)
        } else {
            resReplaceEntryInComponent(componentName, resId, newStringIndex)
        }
    }
}
