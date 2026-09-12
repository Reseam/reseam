// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Plugin
import org.gradle.api.initialization.Settings
import java.io.File

/**
 * Applied in `settings.gradle.kts`. Lays out a bundle from its directories:
 * `apps/<app>/patch` holds patches, `apps/<app>/extensions/<name>` and
 * `shared/<name>` hold extensions, the root packs the bundle. No module needs
 * a build script unless it adds dependencies.
 */
class ReseamWorkspacePlugin : Plugin<Settings> {
    override fun apply(settings: Settings) {
        val root = settings.rootDir
        settings.dependencyResolutionManagement.repositories.apply {
            mavenCentral()
            google { mavenContent { includeGroupAndSubgroups("androidx"); includeGroupAndSubgroups("com.android"); includeGroupAndSubgroups("com.google") } }
            maven { url = settings.providers.provider { java.net.URI(RESEAM_MAVEN) }.get(); mavenContent { includeGroup("app.reseam") } }
        }
        workspace(settings)?.let { settings.includeBuild(it) }

        val patches = mutableListOf<String>()
        val extensions = mutableListOf<String>()
        for (app in directories(File(root, "apps"))) {
            File(app, "patch").takeIf { it.isDirectory }?.let { patches += include(settings, ":apps:${app.name}:patch", it) }
            for (extension in directories(File(app, "extensions"))) {
                extensions += include(settings, ":apps:${app.name}:extensions:${extension.name}", extension)
            }
        }
        for (shared in directories(File(root, "shared"))) {
            extensions += include(settings, ":shared:${shared.name}", shared)
        }

        settings.gradle.beforeProject {
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
        parent.listFiles().orEmpty().filter { it.isDirectory && !it.name.startsWith(".") && it.name != "build" }.sortedBy { it.name }

    /** A checkout of the engine repository whose SDK replaces the published one. */
    private fun workspace(settings: Settings): File? {
        val path = System.getenv("RESEAM_WORKSPACE")?.takeIf { it.isNotBlank() }
            ?: settings.providers.gradleProperty("reseam.workspace").orNull?.takeIf { it.isNotBlank() }
            ?: return null
        return File(path).also { require(it.isDirectory) { "reseam.workspace points to a missing directory: $it" } }
    }

    private companion object {
        const val RESEAM_MAVEN = "https://git.reseam.app/api/packages/reseam/maven"
    }
}
