// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.ExtClass
import app.reseam.patch.Type
import app.reseam.patch.after
import app.reseam.patch.before
import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.opcode
import app.reseam.patch.invoke
import app.reseam.patch.klass
import app.reseam.patch.method
import app.reseam.patch.point
import app.reseam.patch.replace
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.NewField

val replaceFallsThrough =
    fixturePatch(TestStrings.replaceFallsThrough, TestStrings.replaceFallsThroughDescription) {
        anchorHost.method("replaced").replace { whenTrue(bool(true)) { returnVoid() } }
    }

private object GhostExtension : ExtClass("app.reseam.test.GhostExtension") {
    val run by static(Type.Boolean, returns = Type.Boolean)
}

val extMethodMissing =
    fixturePatch(TestStrings.extMethodMissing, TestStrings.extMethodMissingDescription) {
        anchoredTwice.before { call(GhostExtension.run, bool(true)) }
    }

val assignFieldRead =
    fixturePatch(TestStrings.assignFieldRead, TestStrings.assignFieldReadDescription) {
        anchoredTwice.before { staticField(anchorValue).assign(int(7)) }
    }

val afterReturn =
    fixturePatch(TestStrings.afterReturn, TestStrings.afterReturnDescription) {
        anchoredTwice
            .point { opcode(Opcode.RETURN_VOID) }
            .after { callStatic("com.example.Observer", "mark", "(I)V", int(1)) }
    }

val privateFieldAccess =
    fixturePatch(TestStrings.privateFieldAccess, TestStrings.privateFieldAccessDescription) {
        anchorHost.classDef.addField(
            NewField(
                name = "secret",
                fieldType = Type.Int,
                accessFlags = (AccessFlags.PRIVATE or AccessFlags.STATIC).toUInt(),
                initialValue = null,
            )
        )
        klass("com.other.Peer").method("run").before {
            setStatic(FieldRef("Lcom/example/AnchorHost;", "secret", Type.Int), int(1))
        }
    }

val callArity =
    fixturePatch(TestStrings.callArity, TestStrings.callArityDescription) {
        anchoredTwice.before {
            callStatic("java.lang.String", "valueOf", "(I)Ljava/lang/String;")
        }
    }

val wideCondition = invalidCondition(TestStrings.wideCondition) { whenTrue(long(1)) {} }

val primitiveNull = invalidCondition(TestStrings.primitiveNull) { whenNull(int(1)) {} }

val mixedEquality =
    invalidCondition(TestStrings.mixedEquality) { whenEqual(int(1), string("one")) {} }

val primitiveInstance =
    invalidCondition(TestStrings.primitiveInstance) { whenInstanceOf(int(1), Type.Object) {} }

val primitiveInstanceType =
    invalidCondition(TestStrings.primitiveInstanceType) { whenInstanceOf(nullObject, Type.Int) {} }

val primitiveConstruction =
    invalidCondition(TestStrings.primitiveConstruction) { newInstance(Type.Int) }

val invalidConstructorReturn =
    invalidCondition(TestStrings.invalidConstructorReturn) { newInstance(Type.Object, "()I") }
