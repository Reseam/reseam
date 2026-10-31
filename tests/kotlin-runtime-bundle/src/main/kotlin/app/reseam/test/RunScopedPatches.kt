// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.RunList

private val registrations = RunList<String>()

val registrationOwner =
    declaredFixture("registration-owner") {
        enabledByDefault(false)
        afterDependents {
            files.write(
                "assets/registrations.txt",
                registrations.values.joinToString(",").encodeToByteArray(),
            )
        }
    }

val registersAndFails =
    declaredFixture("registers-and-fails") {
        enabledByDefault(false)
        dependsOn(registrationOwner)
        execute {
            registrations.add("failed")
            error("the fixture fails after registering")
        }
    }

val registersAndApplies =
    declaredFixture("registers-and-applies") {
        enabledByDefault(false)
        dependsOn(registrationOwner)
        execute { registrations.add("applied") }
    }
