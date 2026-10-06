// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.*
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.literal
import app.reseam.patch.types.MethodRef

private val queryHost = klass("com.example.PointQueryHost")
private val deferredSingle =
    queryHost
        .method("copied")
        .points {
            invokeStatic { name("take") }
            argument(0) { literal(4) }
        }
        .single()

private object PointObserver : ExtClass("com.example.PointObserver") {
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
            duplicates.forEach {
                if (growFrame) {
                    method.reserveLocal("force relocation", Type.Long)
                    growFrame = false
                }
                captureArgumentAs("value", 0).before { call(PointObserver.take, capture("value")) }
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

val pointRedirects =
    patch("point-redirects") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        execute {
            fun call(name: String) =
                queryHost.method(name).point { invokeStatic { name("compute") } }
            val consumed = call("usedResult")
            check(runCatching { consumed.redirectTo(PointObserver.replacement) }.isFailure)
            val unused = call("unusedResult")
            for (target in
                listOf(
                    PointObserver.wrong,
                    PointObserver.instance,
                    PointObserver.concealed,
                    PointObserver.absent,
                )) {
                check(runCatching { unused.redirectTo(target) }.isFailure) { "accepted $target" }
            }
            unused.redirectTo(PointObserver.replacement)
            val wide = queryHost.method("wide").points { invokeVirtual { name("take") } }.single()
            wide.redirectTo(PointObserver.consume)
            val superCall = queryHost.method("superCall").point { invoke(Opcode.INVOKE_SUPER) }
            check(runCatching { superCall.redirectTo(PointObserver.consume) }.isFailure)
            // Global and point redirects must reject the same consumed-result mismatch.
            check(
                runCatching {
                    bytecode.redirectCalls(
                        MethodRef("Lcom/example/PointObserver;", "compute", "(I)I"),
                        PointObserver.replacement,
                    )
                }
                    .isFailure
            )
            val count =
                bytecode.redirectCalls(
                    MethodRef("Lcom/example/PointObserver;", "take", "(I)V"),
                    PointObserver.replacement,
                )
            check(count == 11) { "redirected $count" }
            check(
                bytecode.redirectCalls(
                    MethodRef("Lcom/example/PointObserver;", "take", "(I)V"),
                    PointObserver.replacement,
                ) == 0
            )
            check(
                klass("com.example.ExtensionCaller")
                    .method("call")
                    .point { invokeStatic { name("take") } }
                    .index == 0
            )
        }
    }

private val prefixedRoutes = methods("prefixedRoutes") { stringsStartingWith("reel/") }

val stringPrefixQueries =
    patch("string-prefix-queries") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        execute {
            check(prefixedRoutes.all.map { it.name }.toSet() == setOf("watch", "create"))
            check(
                klass("com.example.Routes")
                    .methods {
                        stringsStartingWith("reel/")
                        name("embedded")
                    }
                    .all
                    .isEmpty()
            )
            check(
                methods {
                    stringsStartingWith("reel/")
                    includeExtensions()
                }
                    .all
                    .size == 3
            )
            check(methods { stringsStartingWith("absent/") }.all.isEmpty())
            check(runCatching { methods { stringsStartingWith("") }.all }.isFailure)
        }
    }

val high16Literals =
    patch("high16-literals") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        execute {
            for ((name, before, after) in
                listOf(
                    Triple("positive", 67108864L, 134217728L),
                    Triple("negative", -65536L, -131072L),
                    Triple("wide", 1L shl 60, Long.MIN_VALUE),
                )) {
                val target = method { literals(before) }
                check(target.name == name)
                check(target.point { literal(before) }.instruction.literal == before)
                check(target.method.replaceLiteral(before, after))
                check(target.point { literal(after) }.instruction.literal == after)
                check(method { literals(after) }.name == name)
            }
        }
    }
