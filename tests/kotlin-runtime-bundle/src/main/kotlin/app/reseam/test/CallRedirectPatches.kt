// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.*
import app.reseam.patch.dex.Opcode
import app.reseam.patch.types.MethodRef

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
