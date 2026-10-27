// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.types.MethodInfo
import app.reseam.patch.types.MethodRef

val MethodRef.returnType: String
    get() = proto.substring(prototypeReturnStart(proto))

val MethodRef.parameterTypes: List<String>
    get() = parseParameterTypes(proto)

val MethodInfo.returnType: String
    get() = proto.substring(prototypeReturnStart(proto))

val MethodInfo.parameterTypes: List<String>
    get() = parseParameterTypes(proto)

val MethodInfo.isStatic: Boolean
    get() = AccessFlags.STATIC.isSet(accessFlags)

val MethodInfo.descriptor: String
    get() = "$classDescriptor->$methodName$proto"

val MethodRef.descriptor: String
    get() = "$definingClass->$name$proto"

/** The declared parameters of a complete method prototype; malformed descriptors are rejected. */
fun parseParameterTypes(proto: String): List<String> {
    val end = prototypeReturnStart(proto) - 1
    var offset = 1
    return buildList {
        while (offset < end) {
            val next = descriptorEnd(proto, offset)
            add(proto.substring(offset, next))
            offset = next
        }
    }
}

private fun prototypeReturnStart(proto: String): Int {
    require(proto.startsWith('(')) { "Method prototype must start with '(': $proto" }
    var offset = 1
    while (offset < proto.length && proto[offset] != ')') {
        require(proto[offset] != 'V') { "Void is not a parameter type: $proto" }
        offset = descriptorEnd(proto, offset)
    }
    require(offset < proto.length) { "Method prototype has no closing ')': $proto" }
    val returns = offset + 1
    require(descriptorEnd(proto, returns) == proto.length) {
        "Method prototype has trailing data: $proto"
    }
    return returns
}

private fun descriptorEnd(proto: String, start: Int): Int {
    var offset = start
    while (offset < proto.length && proto[offset] == '[') offset++
    require(offset < proto.length) { "Incomplete type at $start in $proto" }
    return when (proto[offset]) {
        'L' -> {
            val end = proto.indexOf(';', offset)
            require(end > offset + 1) { "Incomplete object type at $start in $proto" }
            end + 1
        }
        'Z',
        'B',
        'S',
        'C',
        'I',
        'J',
        'F',
        'D' -> offset + 1
        'V' -> {
            require(offset == start) { "Void cannot be an array element: $proto" }
            offset + 1
        }
        else -> error("Invalid type at $start in $proto")
    }
}

fun registerWordCount(type: String): Int = if (type == "J" || type == "D") 2 else 1

fun isReferenceType(type: String): Boolean = type.startsWith("L") || type.startsWith("[")
