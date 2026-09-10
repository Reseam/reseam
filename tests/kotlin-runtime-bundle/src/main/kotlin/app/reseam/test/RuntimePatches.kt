// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.patch

val finalizeOwner = patch("finalize-owner") {
    description("Exercises afterDependents through the real Kotlin runtime")
    compatibleWith("com.example.test")

    execute {
        log.info("finalize-owner execute")
    }

    afterDependents {
        manifest.addPermission("android.permission.INTERNET")
    }
}

val runtimeApi = patch("runtime-api") {
    description("Exercises PatchRuntime scopes against split APK state")
    compatibleWith("com.example.test")
    val baseVersion = stringOption("baseVersion", default = "2.0-base")
    val splitVersion = stringOption("splitVersion", default = "2.0-split")
    val splitText = stringOption("splitText", default = "Split patched")

    execute {
        manifest.setVersionName(options[baseVersion])
        manifest.component("config.test").setVersionName(options[splitVersion])
        resources.setString("split_label", options[splitText])
        files.write("assets/base-marker.txt", "base".encodeToByteArray())
        files.component("config.test").write("assets/split-marker.txt", "split".encodeToByteArray())
    }
}

val dependentRuntime = patch("dependent-runtime") {
    description("Depends on finalize-owner to trigger afterDependents")
    compatibleWith("com.example.test")
    dependsOn(finalizeOwner)

    execute {
        files.component("config.test").write("assets/dependent-marker.txt", "dependent".encodeToByteArray())
    }
}

val requiredOption = patch("required-option") {
    description("Used to verify option validation against real Kotlin patches")
    compatibleWith("com.example.test")
    enabledByDefault(false)
    val token = stringOption("token", required = true)

    execute {
        log.info(options[token])
    }
}

val internalHelper = patch {
    description("An internal dependency: never listed, runs when something depends on it")
    compatibleWith("com.example.test")

    execute {
        files.write("assets/internal-marker.txt", "internal".encodeToByteArray())
    }
}

val usesInternal = patch("uses-internal") {
    description("Pulls the internal helper in as a dependency")
    compatibleWith("com.example.test")
    dependsOn(internalHelper)

    execute {
        log.info("uses-internal execute")
    }
}
