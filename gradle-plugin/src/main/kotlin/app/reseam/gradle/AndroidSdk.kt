// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.util.Properties
import org.gradle.api.GradleException
import org.gradle.api.Project

internal const val MIN_API = 26

internal val reseamVersion: String by lazy {
    val properties = Properties()
    ReseamPatchesPlugin::class
        .java
        .getResourceAsStream("/META-INF/reseam.properties")
        .let { it ?: throw GradleException("Reseam plugin version metadata is missing") }
        .use(properties::load)
    properties.getProperty("version")
        ?: throw GradleException("Reseam plugin version metadata has no version")
}

internal object AndroidSdk {
    private fun home(): File {
        val path =
            System.getenv("ANDROID_HOME")
                ?: System.getenv("ANDROID_SDK_ROOT")
                ?: throw GradleException("ANDROID_HOME is not set; it is needed for android.jar")
        return File(path).takeIf { it.isDirectory }
            ?: throw GradleException("ANDROID_HOME points to a missing directory: $path")
    }

    fun platformJar(): File =
        File(home(), "platforms")
            .listFiles()
            .orEmpty()
            .filter { it.name.startsWith("android-") && File(it, "android.jar").isFile }
            .maxByOrNull { it.name.removePrefix("android-").toIntOrNull() ?: -1 }
            ?.resolve("android.jar")
            ?: throw GradleException("No platforms/android-*/android.jar under ${home()}")
}

/**
 * Artifact stem used by patch and extension modules; workspace layouts set their own convention.
 */
abstract class ReseamArtifactExtension {
    abstract val name: org.gradle.api.provider.Property<String>
}

internal fun Project.reseamArtifact(): ReseamArtifactExtension =
    extensions.findByType(ReseamArtifactExtension::class.java)
        ?: extensions.create("reseamArtifact", ReseamArtifactExtension::class.java).also {
            it.name.convention(
                path.split(':').filter(String::isNotEmpty).joinToString("-").ifEmpty { name }
            )
        }
