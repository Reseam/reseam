// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.FieldRefMatch
import app.reseam.patch.MethodRefMatch
import app.reseam.patch.descriptor
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

internal class MethodRefMatchSpec : MethodRefMatch {
    var owner: String? = null
        private set

    var name: String? = null
        private set

    var returnType: String? = null
        private set

    var parameters: List<String>? = null
        private set

    private val assignableOwners = mutableListOf<String>()
    private val required = mutableListOf<String>()
    val requiredParameters: List<String>
        get() = required

    var parameterCount: Int? = null
        private set

    val needsPostFilter
        get() = assignableOwners.isNotEmpty()

    override fun owner(type: String) {
        owner = descriptor(type)
    }

    override fun ownerAssignableTo(type: String) {
        assignableOwners += descriptor(type)
    }

    override fun name(value: String) {
        name = value
    }

    override fun returns(type: String) {
        returnType = descriptor(type)
    }

    override fun params(vararg types: String) {
        parameters = types.map(::descriptor)
    }

    override fun hasParam(type: String) {
        required += descriptor(type)
    }

    override fun paramCount(count: Int) {
        parameterCount = count
    }

    fun matches(ref: MethodRef): Boolean {
        if (owner != null && ref.definingClass != owner || name != null && ref.name != name)
            return false
        if (returnType != null && ref.returnType != returnType) return false
        if (assignableOwners.any { !isAssignableType(ref.definingClass, it) }) return false
        if (parameters != null || requiredParameters.isNotEmpty() || parameterCount != null) {
            val actual = ref.parameterTypes
            if (
                parameters != null && actual != parameters ||
                    parameterCount != null && actual.size != parameterCount
            )
                return false
            if (requiredParameters.any { it !in actual }) return false
        }
        return true
    }
}

internal class FieldRefMatchSpec : FieldRefMatch {
    var owner: String? = null
        private set

    var name: String? = null
        private set

    var fieldType: String? = null
        private set

    override fun owner(type: String) {
        owner = descriptor(type)
    }

    override fun name(value: String) {
        name = value
    }

    override fun type(descriptor: String) {
        fieldType = descriptor(descriptor)
    }

    fun matches(ref: FieldRef) =
        (owner == null || ref.definingClass == owner) &&
            (name == null || ref.name == name) &&
            (fieldType == null || ref.fieldType == fieldType)
}
