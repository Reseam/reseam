// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.*
import app.reseam.patch.dex.literal
import app.reseam.patch.types.MethodRef

internal val queryHost = klass("com.example.PointQueryHost")

private val deferredSingle =
    queryHost
        .method("copied")
        .points {
            invokeStatic { name("take") }
            argument(0) { literal(4) }
        }
        .single()

internal object PointObserver : ExtClass("com.example.PointObserver") {
    val take by static(Type.Int)
    val replacement by static(Type.Int)
    val wrong by static(Type.String)
    val instance by method(Type.Int)
    val concealed by static(Type.Int)
    val absent by static(Type.Int)
    val consume by static(Type.Object, Type.Long, Type.Int)
}

val pointQueries =
    patch("point-queries") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        execute {
            // References are indexed even for external owners and overloaded methods.
            val intCalls = methods {
                calls {
                    owner("com.example.PointObserver")
                    name("take")
                    params(Type.Int)
                    returns(Type.Void)
                }
            }
                .all
                .map { it.name }
                .toSet()
            check(
                methods { calls(MethodRef("Lcom/example/PointObserver;", "take", "(I)V")) }
                    .all
                    .map { it.name }
                    .toSet() == intCalls
            )
            check(
                "copied" in intCalls &&
                    "duplicates" in intCalls &&
                    "unknown" !in intCalls &&
                    "wideCopied" !in intCalls
            )
            check("call" !in intCalls) // Extension callers are hidden by default.
            check(
                methods {
                    calls {
                        owner("com.example.PointObserver")
                        name("take")
                        params(Type.Int)
                        returns(Type.Void)
                    }
                    includeExtensions()
                }
                    .all
                    .map { it.name }
                    .toSet() == intCalls + "call"
            )
            check(
                methods {
                    calls {
                        owner("java.lang.Object")
                        name("take")
                        params(Type.Long, Type.Int)
                    }
                }
                    .all
                    .map { it.name }
                    .toSet() == setOf("wide", "superCall")
            )
            check(
                methods {
                    calls {
                        name("take")
                        params()
                        returns(Type.Void)
                    }
                }
                    .all
                    .isEmpty()
            )
            check(
                methods {
                    calls {
                        owner("com.example.Missing")
                        name("take")
                    }
                }
                    .all
                    .isEmpty()
            )
            check(
                methods {
                    calls {
                        params(Type.Int)
                        returns(Type.Int)
                    }
                }
                    .all
                    .map { it.name }
                    .toSet() == setOf("usedResult", "unusedResult", "unknown", "exception")
            )
            check(
                queryHost
                    .methods {
                        name("copied")
                        calls {
                            name("take")
                            hasParam(Type.Int)
                            paramCount(1)
                        }
                    }
                    .all
                    .single()
                    .name == "copied"
            )
            check(
                methods {
                    calls {
                        name("take")
                        hasParam(Type.Long)
                        paramCount(2)
                    }
                }
                    .all
                    .map { it.name }
                    .toSet() == setOf("wide", "superCall")
            )
            check(runCatching { methods { calls { paramCount(1) } }.all }.isFailure)
            check(deferredSingle.index == 2)
            val report = mutableListOf<String>()
            for (name in
                listOf(
                    "copied",
                    "same",
                    "conflict",
                    "passed",
                    "wide",
                    "wideCopied",
                    "wideBroken",
                    "exception",
                    "duplicates",
                    "overwritten",
                    "unknown",
                )) {
                val points =
                    queryHost.method(name).points {
                        invoke { name("take") }
                        argument(if (name == "wide") 2 else 0) { literal(4) }
                    }
                report += "$name:${points.all.size}"
                if (name == "passed") check(points.explain().reasons.any { "incoming" in it })
                if (name == "unknown") check(points.explain().reasons.any { "unknown" in it })
            }
            val duplicates =
                queryHost.method("duplicates").points {
                    invokeStatic { name("take") }
                    argument(0) { literal(4) }
                }
            check(runCatching { duplicates.single().index }.isFailure)
            val mismatch = runCatching {
                queryHost
                    .method("conflict")
                    .points {
                        invokeStatic()
                        argument(0) { literal(4) }
                    }
                    .single()
                    .index
            }
            check(mismatch.isFailure)
            // Selection is completed before the first hook changes the frame and inserts invokes.
            var growFrame = true
            duplicates.all.forEach { point ->
                if (growFrame) {
                    point.method.reserveLocal("force relocation", Type.Long)
                    growFrame = false
                }
                point.captureArgumentAs("value", 0).before {
                    call(PointObserver.take, capture("value"))
                }
            }
            val combined =
                queryHost
                    .methods { name("copied") }
                    .points {
                        invokeStatic { name("take") }
                        argument(0) { literal(4) }
                    }
            check(combined.all.size == 1)
            val unused = queryHost.method("unusedResult").point { invokeStatic { name("compute") } }
            check(runCatching { unused.next { resultOf(Type.Int) }.index }.isFailure)
            check(
                queryHost
                    .method("usedResult")
                    .point { invokeStatic { name("compute") } }
                    .next { resultOf(Type.Int) }
                    .index == 2
            )
            files.write("assets/point-queries.txt", report.joinToString("\n").encodeToByteArray())
        }
    }
