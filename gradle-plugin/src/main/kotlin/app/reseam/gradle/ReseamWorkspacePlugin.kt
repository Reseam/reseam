// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import org.gradle.api.Plugin
import org.gradle.api.initialization.Settings

internal class ReseamWorkspacePlugin : Plugin<Settings> {
    override fun apply(settings: Settings) {
        val root = settings.rootDir
        settings.dependencyResolutionManagement.repositories.apply {
            mavenCentral()
            google {
                mavenContent {
                    includeGroupAndSubgroups("androidx")
                    includeGroupAndSubgroups("com.android")
                    includeGroupAndSubgroups("com.google")
                }
            }
            maven {
                url = settings.providers.provider { java.net.URI(RESEAM_MAVEN) }.get()
                mavenContent { includeGroup("app.reseam") }
            }
        }
        workspace(settings)?.let { settings.includeBuild(it) }

        val patches = mutableMapOf<String, String>()
        val extensions = mutableMapOf<String, String>()
        for (app in directories(File(root, "apps"))) {
            File(app, "patch")
                .takeIf { it.isDirectory }
                ?.let { patches[include(settings, ":apps:${app.name}:patch", it)] = app.name }
            for (extension in directories(File(app, "extensions"))) {
                extensions[
                    include(
                        settings,
                        ":apps:${app.name}:extensions:${extension.name}",
                        extension,
                    )] = "${app.name}-${extension.name}"
            }
        }
        // A shared folder is an extension, patches for several apps in `patch/`, or both.
        for (shared in directories(File(root, "shared"))) {
            val patch = File(shared, "patch").takeIf { it.isDirectory }
            if (patch == null || File(shared, "src").isDirectory) {
                extensions[include(settings, ":shared:${shared.name}", shared)] = shared.name
            }
            patch?.let {
                patches[include(settings, ":shared:${shared.name}:patch", it)] = shared.name
            }
        }

        require(
            (patches.values.map { "$it-patches.jar" } +
                    extensions.values.map { "$it.dex" } +
                    "d8-globals.dex")
                .let { it.size == it.toSet().size }
        ) {
            "workspace artifact names must be unique"
        }
        settings.gradle.beforeProject {
            (patches[path] ?: extensions[path])?.let { reseamArtifact().name.set(it) }
            when (path) {
                ":" -> pluginManager.apply(ReseamBundlePlugin::class.java)
                in patches -> pluginManager.apply(ReseamPatchesPlugin::class.java)
                in extensions -> pluginManager.apply(ReseamExtensionPlugin::class.java)
            }
        }
    }

    private fun include(settings: Settings, path: String, dir: File): String {
        settings.include(path)
        settings.project(path).projectDir = dir
        return path
    }

    private fun directories(parent: File): List<File> =
        (if (!parent.exists()) emptyArray()
            else
                parent.listFiles()
                    ?: throw org.gradle.api.GradleException(
                        "cannot read workspace directory $parent"
                    ))
            .filter { it.isDirectory && !it.name.startsWith(".") && it.name != "build" }
            .sortedBy { it.name }

    private fun workspace(settings: Settings): File? {
        val path =
            System.getenv("RESEAM_WORKSPACE")?.takeIf { it.isNotBlank() }
                ?: settings.providers.gradleProperty("reseam.workspace").orNull?.takeIf {
                    it.isNotBlank()
                }
                ?: return null
        return File(path).also {
            require(it.isDirectory) { "reseam.workspace points to a missing directory: $it" }
        }
    }

    private companion object {
        const val RESEAM_MAVEN = "https://git.reseam.app/api/packages/reseam/maven"
    }
}
