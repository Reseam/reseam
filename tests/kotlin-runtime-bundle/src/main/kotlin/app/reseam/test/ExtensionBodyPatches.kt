// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.ExtClass
import app.reseam.patch.PatchRuntime
import app.reseam.patch.after
import app.reseam.patch.klass
import app.reseam.patch.method

/** An extension whose `filled`, `missing`, and `unused` are `external`; `entry` calls `missing`. */
private object Bodies : ExtClass("app.reseam.test.Bodies") {
    val entry by static()
    val filled by static()
    val missing by static()
}

private fun PatchRuntime.callBodies() {
    klass("com.example.HookTarget").method("invokeGrowth").after {
        Bodies.entry()
        Bodies.filled()
    }
}

val unfilledBodies =
    fixturePatch(TestStrings.unfilledBodies, TestStrings.unfilledBodiesDescription) { callBodies() }

val filledBodies =
    fixturePatch(TestStrings.filledBodies, TestStrings.filledBodiesDescription) {
        callBodies()
        Bodies.filled.implement { returnVoid() }
        Bodies.missing.implement { returnVoid() }
    }
