// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.after
import app.reseam.patch.before
import app.reseam.patch.invoke
import app.reseam.patch.method
import app.reseam.patch.point
import app.reseam.patch.skipWhen

val skipWhenCall =
    fixturePatch(TestStrings.skipWhen, TestStrings.skipWhenDescription) {
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
    fixturePatch(TestStrings.beforeJoin, TestStrings.beforeJoinDescription) {
        skipHost
            .method("joinedBefore")
            .point { literal(2) }
            .next { invokeStatic { name("mark") } }
            .next { invokeStatic { name("mark") } }
            .before { mark(9) }
    }

val jumpSources =
    fixturePatch(TestStrings.jumpSources, TestStrings.jumpSourcesDescription) {
        val start = skipHost.method("joinedBefore").point { literal(2) }
        val joined =
            start.next { invokeStatic { name("mark") } }.next { invokeStatic { name("mark") } }
        val none = runCatching { start.jumpSource().index }
        files.write(
            "assets/jump-sources.txt",
            "${joined.jumpSource().index}\n${none.isFailure}".encodeToByteArray(),
        )
    }

val jumpToPoint =
    fixturePatch(TestStrings.jumpToPoint, TestStrings.jumpToPointDescription) {
        val method = skipHost.method("joinedBefore")
        val target = method.point { literal(3) }
        target.before { mark(8) }
        method.point { literal(4) }.before { jumpTo(target) }
        target.before { mark(9) }
    }

val afterOrder =
    fixturePatch(TestStrings.afterOrder, TestStrings.afterOrderDescription) {
        val point = skipHost.method("joinedBefore").point { literal(3) }
        point.after { mark(5) }
        point.after { mark(6) }
    }
