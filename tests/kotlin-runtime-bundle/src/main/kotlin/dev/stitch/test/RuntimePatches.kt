// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.compatibleWith
import app.reseam.patch.optionsOf
import app.reseam.patch.patch
import app.reseam.patch.stringOption

val finalizeOwner = patch(
    name = "finalize-owner",
    description = "Exercises afterDependents through the real Kotlin runtime"
) {
    compatibleWith("com.example.test")

    execute { ctx ->
        ctx.log.info("finalize-owner execute")
    }

    afterDependents { ctx ->
        ctx.manifest.addPermission("android.permission.INTERNET")
    }
}

val runtimeApi = patch(
    name = "runtime-api",
    description = "Exercises PatchRuntime scopes against split APK state",
    compatibleWith = listOf(compatibleWith("com.example.test")),
    options = optionsOf(
        stringOption("baseVersion", default = "2.0-base"),
        stringOption("splitVersion", default = "2.0-split"),
        stringOption("splitText", default = "Split patched"),
    ),
) {
    execute { ctx ->
        ctx.manifest.setVersionName(ctx.options.string("baseVersion")!!)
        ctx.manifest.component("config.test").setVersionName(ctx.options.string("splitVersion")!!)
        ctx.resources.setString("split_label", ctx.options.string("splitText")!!)
        ctx.files.write("assets/base-marker.txt", "base".encodeToByteArray())
        ctx.files.component("config.test")
            .write("assets/split-marker.txt", "split".encodeToByteArray())
    }
}

val dependentRuntime = patch(
    name = "dependent-runtime",
    description = "Depends on finalize-owner to trigger afterDependents",
    compatibleWith = listOf(compatibleWith("com.example.test")),
    dependsOn = listOf(finalizeOwner),
) {
    execute { ctx ->
        ctx.files.component("config.test")
            .write("assets/dependent-marker.txt", "dependent".encodeToByteArray())
    }
}

val requiredOption = patch(
    name = "required-option",
    description = "Used to verify option validation against real Kotlin patches",
    compatibleWith = listOf(compatibleWith("com.example.test")),
    enabledByDefault = false,
    options = optionsOf(
        stringOption("token", required = true),
    ),
) {
    execute { ctx ->
        ctx.log.info(ctx.options.string("token")!!)
    }
}
