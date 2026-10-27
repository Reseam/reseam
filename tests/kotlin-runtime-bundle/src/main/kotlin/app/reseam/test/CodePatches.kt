// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.Type
import app.reseam.patch.addMethod
import app.reseam.patch.before
import app.reseam.patch.invoke
import app.reseam.patch.klass
import app.reseam.patch.method
import app.reseam.patch.replace
import app.reseam.patch.reserveLocal

val embeddedStrings =
    fixturePatch("embedded-strings") {
        val contained =
            bytecode.replaceStringsContaining("com.google.android.gsf") { old ->
                old.takeIf { it.startsWith("content://") }
                    ?.replace("com.google.android.gsf", "app.reseam.gsf")
            }
        val prefixed =
            bytecode.replaceStringsStartingWith("com.google.android.gsf.") { old ->
                old.replace(".action.", ".intent.")
            }
        files.write("assets/embedded-strings.txt", "$contained|$prefixed".encodeToByteArray())
    }

val whenInstanceOfBranch =
    fixturePatch("when-instance-of") {
        skipHost.method("typed").replace {
            whenInstanceOf(param(0), "java.lang.String") { mark(1) } otherwise { mark(2) }
            returnVoid()
        }
    }

val castParameter =
    fixturePatch("cast-parameter") {
        skipHost.method("typed").before { whenNotNull(param(0).cast("java.lang.String")) {} }
    }

val replaceBothReturn =
    fixturePatch("replace-both-return") {
        anchorHost.method("replaced").replace {
            whenTrue(param(0)) { returnValue(int(10)) } otherwise { returnValue(int(20)) }
        }
    }

val typedComparisons =
    fixturePatch("typed-comparisons") {
        val host = klass("com.example.ComparisonHost")
        for (type in listOf("long", "float", "double", "int", "reference")) {
            host.method("${type}Equal").replace {
                whenEqual(param(0), param(1)) { returnValue(int(1)) } otherwise
                    {
                        returnValue(int(0))
                    }
            }
            host.method("${type}NotEqual").replace {
                whenNotEqual(param(0), param(1)) { returnValue(int(1)) } otherwise
                    {
                        returnValue(int(0))
                    }
            }
        }
    }

val incompleteHierarchies =
    fixturePatch("incomplete-hierarchies") {
        val host = klass("com.example.TypeHost")
        for ((name, expected) in
            listOf(
                "application" to "Landroid/content/Context;",
                "image" to "Landroid/view/View;",
                "appView" to "Landroid/view/View;",
                "libraryActivity" to "Landroid/content/Context;",
                "missingSource" to "Lcom/example/KnownTarget;",
                "missingTarget" to "Lcom/example/ExternalTarget;",
                "knownSubclass" to "Lcom/example/KnownSource;",
            )) {
            host.method(name).before {
                callStatic("com.example.Observer", "accept", "($expected)V", param(0))
            }
        }
        val rejected =
            listOf(
                    "unrelated" to "Lcom/example/KnownTarget;",
                    "primitiveReference" to "Landroid/content/Context;",
                    "wideNarrow" to Type.Int,
                    "arrayElements" to "[J",
                )
                .map { (name, expected) ->
                    runCatching {
                        host.method(name).before {
                            callStatic(
                                "com.example.Observer",
                                "accept",
                                "($expected)V",
                                param(0),
                            )
                        }
                    }
                        .isFailure
                }
        files.write(
            "assets/type-validation.txt",
            rejected.joinToString("|").encodeToByteArray(),
        )
    }

val constantForms =
    fixturePatch("constant-forms") {
        val host = klass("com.example.ConstantHost")
        for ((name, value) in
            listOf(
                "small" to 7,
                "shortMax" to 32767,
                "shortMin" to -32768,
                "positiveWide" to 32768,
                "negativeWide" to -32769,
                "positiveHigh" to 65536,
                "negativeHigh" to -65536,
                "color" to -16777216,
                "minimum" to Int.MIN_VALUE,
                "maximum" to Int.MAX_VALUE,
            )) {
            host.method(name).replace {
                whenTrue(param(0)) { returnValue(int(value)) }
                returnValue(int(0))
            }
        }
    }

val longGoto =
    fixturePatch("long-goto") {
        klass("com.example.ConstantHost").method("small").replace {
            whenTrue(param(0)) {} otherwise { repeat(130) { int(1) } }
            returnValue(int(0))
        }
    }

val distantOperands =
    fixturePatch("distant-operands") {
        val target = klass("com.example.HookTarget").method("invokeGrowth")
        repeat(4) { target.reserveLocal("distance$it", Type.Long) }
        target.before {
            whenEqual(param(1), param(2)) {
                callStatic("com.example.Observer", "entry", "(I)V", param(3))
            }
        }
    }

val addedMethod =
    fixturePatch("added-method") {
        val app = klass("com.example.App")
        repeat(2) {
            app.addMethod("reseamCallsBase", "()V") {
                thisObject.callSuper("com.example.AppBase", "onCreate", "()V")
                returnVoid()
            }
        }
        check(app.classDef.methods.count { it.name == "reseamCallsBase" } == 1)
    }
