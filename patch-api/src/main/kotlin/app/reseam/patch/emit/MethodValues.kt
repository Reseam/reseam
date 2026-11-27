// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.MethodLocal
import app.reseam.patch.Type
import app.reseam.patch.descriptor as descriptorOf
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.dex.returnType
import app.reseam.patch.types.CodePlacement
import app.reseam.patch.types.CodeRegister

internal fun CodeEmitter.thisObject(): Value {
    require(!isStaticMethod) { "thisObject is not available in static ${info.descriptor}" }
    return parameter(0, info.classDescriptor)
}

internal fun CodeEmitter.param(index: Int): Value {
    val params = method.parameterTypes
    require(index in params.indices) {
        "Parameter index $index out of bounds for ${info.descriptor}"
    }
    val offset = (if (isStaticMethod) 0 else 1) + params.take(index).sumOf(::registerWordCount)
    return parameter(offset, params[index])
}

internal fun CodeEmitter.paramOfType(type: String): Value {
    val wanted = descriptorOf(type)
    val index = method.parameterTypes.indexOf(wanted)
    require(index >= 0) { "No parameter of type $wanted in ${info.descriptor}" }
    return param(index)
}

internal fun CodeEmitter.result(): Value {
    require(placement == CodePlacement.BeforeReturns && info.returnType != Type.Void) {
        "result exists only in a method-level after hook of a non-void method, not in ${info.descriptor}"
    }
    return Value(this, CodeRegister.Returned, info.returnType)
}

internal fun CodeEmitter.capture(name: String): Value {
    val capture =
        captures.firstOrNull { it.name == name }
            ?: error(
                "No capture named '$name' here; available: ${captures.joinToString { it.name }.ifEmpty { "none" }}"
            )
    return fixed(capture.register, capture.type)
}

internal fun CodeEmitter.local(slot: MethodLocal): Value {
    require(placement != CodePlacement.Body) {
        "$slot is a register of the original body, which this block replaces"
    }
    require(!slot.lost) {
        "$slot no longer exists: the method body was replaced after it was reserved"
    }
    require(slot.method.handle == method.handle) {
        "$slot is a local of another method, not ${info.descriptor}"
    }
    return fixed(slot.register, slot.type)
}
