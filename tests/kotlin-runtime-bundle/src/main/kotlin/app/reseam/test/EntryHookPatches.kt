// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.Type
import app.reseam.patch.after
import app.reseam.patch.alwaysReturn
import app.reseam.patch.before
import app.reseam.patch.reserveLocal

val entryHookOwner =
    declaredFixture(TestStrings.entryHookOwner, null) {
        enabledByDefault(false)
        execute {
            val carried = anchoredTwice.reserveLocal("carried", Type.Int)
            anchoredTwice.before {
                local(carried).assign(int(77))
                mark(10)
            }
            anchoredTwice.before { mark(20) }
            anchoredTwice.reserveLocal("lateInitializer", Type.Long)
            anchoredTwice.before {
                callStatic("com.example.Observer", "scalar", "(I)V", local(carried))
            }
        }
        afterDependents { anchoredTwice.before { mark(40) } }
    }

val entryHookDependent =
    declaredFixture(TestStrings.entryHookDependent, null) {
        enabledByDefault(false)
        dependsOn(entryHookOwner)
        execute { anchoredTwice.before { mark(30) } }
    }

val entryHookFailure =
    declaredFixture(TestStrings.entryHookFailure, null) {
        enabledByDefault(false)
        dependsOn(entryHookOwner)
        execute {
            anchoredTwice.before { mark(99) }
            error("the fixture fails after adding an entry hook")
        }
    }

val entryHookReplacement =
    declaredFixture(TestStrings.entryHookReplacement, null) {
        enabledByDefault(false)
        execute {
            anchoredTwice.before { mark(10) }
            anchoredTwice.alwaysReturn()
            anchoredTwice.before { mark(20) }
            anchoredTwice.after { mark(30) }
            anchoredTwice.before { mark(25) }
        }
    }

val conditionalHooks =
    declaredFixture(TestStrings.conditionalHooks, null) {
        enabledByDefault(false)
        execute {
            anchoredTwice.before { whenTrue(bool(false)) { mark(10) } }
            anchoredTwice.before { mark(20) }
            anchoredTwice.after { whenTrue(bool(false)) { mark(30) } }
            anchoredTwice.after { mark(40) }
        }
    }
