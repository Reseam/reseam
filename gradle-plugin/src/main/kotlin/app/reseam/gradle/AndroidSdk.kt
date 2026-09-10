// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.GradleException
import org.gradle.api.Project
import java.io.File
import java.util.Properties

/** Bytecode below this API level is not produced; it is the floor the engine's runtime targets. */
const val MIN_API = 26

internal val reseamVersion: String by lazy {
    val properties = Properties()
    ReseamPatchesPlugin::class.java.getResourceAsStream("/META-INF/reseam.properties")!!.use(properties::load)
    properties.getProperty("version")
}

internal object AndroidSdk {
    private fun home(): File {
        val path = System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT")
            ?: throw GradleException("ANDROID_HOME is not set; it is needed for android.jar and d8")
        return File(path).takeIf { it.isDirectory }
            ?: throw GradleException("ANDROID_HOME points to a missing directory: $path")
    }

    fun platformJar(): File =
        File(home(), "platforms").listFiles().orEmpty()
            .filter { it.name.startsWith("android-") && File(it, "android.jar").isFile }
            .maxByOrNull { it.name.removePrefix("android-").toIntOrNull() ?: -1 }
            ?.resolve("android.jar")
            ?: throw GradleException("No platforms/android-*/android.jar under ${home()}")

    fun d8(): File {
        System.getenv("D8_BIN")?.takeIf { it.isNotBlank() }?.let { return File(it) }
        return File(home(), "build-tools").listFiles().orEmpty()
            .filter { File(it, "d8").isFile }
            .maxByOrNull { it.name }
            ?.resolve("d8")
            ?: throw GradleException("No build-tools/*/d8 under ${home()}")
    }
}

/** `apps/telegram/extensions/anti-delete` as `telegram-anti-delete`; `shared/settings-runtime` as `settings-runtime`. */
internal fun Project.reseamArtifactName(): String =
    path.split(':').filter { it.isNotEmpty() && it !in setOf("apps", "extensions", "shared", "patch") }.joinToString("-")
