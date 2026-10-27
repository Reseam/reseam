// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.FieldRefMatch
import app.reseam.patch.MethodRefMatch
import app.reseam.patch.descriptor
import app.reseam.patch.types.FieldPattern
import app.reseam.patch.types.MethodPattern

internal class MethodRefMatchSpec : MethodRefMatch {
    var owner: String? = null
        private set

    var name: String? = null
        private set

    var returnType: String? = null
        private set

    var parameters: List<String>? = null
        private set

    private val required = mutableListOf<String>()
    private val assignableOwners = mutableListOf<String>()
    private var parameterCount: Int? = null

    val pattern: MethodPattern
        get() =
            MethodPattern(
                owner,
                name,
                returnType,
                parameters,
                required.toList(),
                parameterCount?.toUInt(),
                assignableOwners.toList(),
            )

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
}

internal class FieldRefMatchSpec : FieldRefMatch {
    private var owner: String? = null
    private var name: String? = null
    private var fieldType: String? = null

    val pattern: FieldPattern
        get() = FieldPattern(owner, name, fieldType)

    override fun owner(type: String) {
        owner = descriptor(type)
    }

    override fun name(value: String) {
        name = value
    }

    override fun type(descriptor: String) {
        fieldType = descriptor(descriptor)
    }
}
