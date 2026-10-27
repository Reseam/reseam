// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.Type
import app.reseam.patch.after
import app.reseam.patch.alwaysReturn
import app.reseam.patch.before
import app.reseam.patch.invoke
import app.reseam.patch.klass
import app.reseam.patch.method
import app.reseam.patch.point
import app.reseam.patch.reserveLocal
import app.reseam.patch.skipWhen

val afterEntryValues =
    fixturePatch("after-entry-values") {
        val target = klass("com.example.HookTarget")
        repeat(5) { target.method("invokeGrowth").reserveLocal("growth$it", Type.Long) }
        target.method("invokeGrowth").after {
            callStatic("com.example.Observer", "entry", "(I)V", param(3))
        }
        val receiver = target.method("receiver").method
        val noRegister = runCatching {
            receiver.findFreeRegister(0, (0 until receiver.registersSize).toList())
        }
        check(noRegister.isFailure)
        target.method("temporaryReuse").after {
            val held = long(0x123456789abcdef0L)
            repeat(32) {
                callStatic("com.example.Observer", "wide", "(J)V", long(it.toLong()))
                callStatic("com.example.Observer", "scalar", "(I)V", int(it))
            }
            whenTrue(param(0)) {
                callStatic("com.example.Observer", "wide", "(J)V", held)
            } otherwise
                {
                    callStatic("com.example.Observer", "wide", "(J)V", held)
                }
            result.assign(held)
        }
        target.method("getFeatureSwitchValue").after {
            val marker = int(42)
            callStatic(
                "com.example.Observer",
                "record",
                "(ILjava/lang/String;JDLjava/lang/String;Ljava/lang/String;Ljava/lang/Object;)V",
                marker,
                param(0),
                paramOfType(Type.Long),
                param(2),
                lastParam,
                paramOfType(Type.String),
                result,
            )
        }
        target.method("receiver").after {
            callStatic(
                "com.example.Observer",
                "receiver",
                "(Lcom/example/HookTarget;)V",
                thisObject,
            )
        }
        target.method("resultOnly").after {
            result.assign(int(42))
        }
    }

val skipWhenCall =
    fixturePatch("skip-when") {
        skipHost
            .method("straight")
            .point { literal(2) }
            .next { invokeStatic { name("mark") } }
            .skipWhen { bool(true) }
        skipHost
            .method("joined")
            .point { literal(2) }
            .next { invokeStatic { name("mark") } }
            .next { invokeStatic { name("mark") } }
            .skipWhen { bool(true) }
        skipHost
            .method("joinedKept")
            .point { literal(2) }
            .next { invokeStatic { name("mark") } }
            .next { invokeStatic { name("mark") } }
            .skipWhen { bool(false) }
    }

val beforeJoin =
    fixturePatch("before-join") {
        skipHost
            .method("joinedBefore")
            .point { literal(2) }
            .next { invokeStatic { name("mark") } }
            .next { invokeStatic { name("mark") } }
            .before { mark(9) }
    }

val pointAnchors =
    fixturePatch("point-anchors") {
        anchoredPoint.after { mark(10) }
        anchoredPoint.after { mark(20) }
        anchoredPoint.before { mark(30) }
        anchoredTwice.before { mark(40) }
        anchoredPoint.after { mark(50) }

        val replaced = anchorHost.method("replaced")
        val lostPoint = replaced.point { literal(2) }
        replaced.alwaysReturn()
        replaced.reserveLocal("afterReplacement", Type.Int)
        val lost = runCatching { lostPoint.before { mark(60) } }
        check(lost.isFailure)
    }

val reservedLocals =
    fixturePatch("reserved-locals") {
        val carried = carry.reserveLocal("carried", Type.Int)
        val carriedWide = carry.reserveLocal("carriedWide", Type.Long)
        carry.before {
            callStatic("com.example.Observer", "scalar", "(I)V", local(carried))
            local(carried).assign(int(77))
            local(carriedWide).assign(long(0x1_0000_0001L))
        }
        carry
            .point { literal(8) }
            .after {
                callStatic("com.example.Observer", "pair", "(II)V", int(1), int(2))
            }
        carry
            .point { literal(9) }
            .after {
                callStatic("com.example.Observer", "scalar", "(I)V", local(carried))
                callStatic("com.example.Observer", "wide", "(J)V", local(carriedWide))
            }
    }

val capturedArguments =
    fixturePatch("captured-arguments") {
        greetCall
            .captureArgumentAs("host", 0)
            .captureArgumentAs("text", 1)
            .captureArgumentAs("count", 2)
            .before {
                callStatic(
                    "com.example.Observer",
                    "seen",
                    "(Lcom/example/ArgHost;Ljava/lang/String;I)V",
                    capture("host"),
                    capture("text"),
                    capture("count"),
                )
            }
        wideCall.captureArgumentAs("long", 1).captureArgumentAs("tail", 2).before {
            callStatic(
                "com.example.Observer",
                "wideSeen",
                "(JI)V",
                capture("long"),
                capture("tail"),
            )
        }
        val notInvoke = runCatching {
            argHost.method("run").point { literal(5) }.captureArgumentAs("x", 0).index
        }
        check(notInvoke.isFailure)
        val outOfRange = runCatching { greetCall.captureArgumentAs("x", 3).index }
        check(outOfRange.isFailure)
    }

val registerWriters =
    fixturePatch("register-writers") {
        val single = takeCall("single").writer(0)
        single.after { mark(99) }
        val copied = takeCall("copied").writer(0)
        copied.after { mark(98) }
        val merged = runCatching { takeCall("merged").writer(0).index }
        val passed = runCatching { takeCall("passed").writer(0).index }
        val copiedParameter = runCatching { takeCall("copiedParameter").writer(0).index }
        files.write(
            "assets/register-writers.txt",
            listOf(merged.isFailure, passed.isFailure, copiedParameter.isFailure)
                .joinToString("\n")
                .encodeToByteArray(),
        )
    }
