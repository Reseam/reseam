// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import org.gradle.api.Plugin
import org.gradle.api.artifacts.ProjectDependency
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

        val folders =
            directories(File(root, "apps")).map { settings.appFolder(it) } +
                directories(File(root, "shared")).map { settings.sharedFolder(it) }
        val typed = folders.filter { it.typed }
        // The root resolves through these; declaring its own repositories would drop them.
        if (typed.isNotEmpty())
            settings.dependencyResolutionManagement.repositories.npmRegistry(
                packages = typed.flatMap { typePackages(it.dir) }.map { it.first.name }.toSet(),
                scopes = setOf(TypeScript.SCOPE),
            )
        val artifacts = folders.flatMap { it.artifacts() } + "d8-globals.dex" + "kotlin.dex"
        require(artifacts.size == artifacts.toSet().size) {
            "workspace artifact names must be unique"
        }
        settings.gradle.beforeProject {
            if (path == ":") {
                pluginManager.apply(ReseamBundlePlugin::class.java)
                typed.forEach { folder ->
                    registerTypecheck(
                        folder.dir,
                        folder.name,
                        folder.extensions.filterNot { it.javascript }.map { project(it.path) },
                    )
                }
                return@beforeProject
            }
            val folder = folders.firstOrNull { path in it.paths() } ?: return@beforeProject
            reseamArtifact().name.set(folder.artifact(path))
            when (path) {
                folder.patch -> {
                    pluginManager.apply(ReseamPatchesPlugin::class.java)
                    folder.settings?.let { dependencies.add("api", project(it)) }
                    folder.extensions.forEach {
                        val variant =
                            if (it.javascript) ModuleVariant.JAVASCRIPT else ModuleVariant.EXPORTS
                        dependencies.add(variant.dependencies, project(it.path))
                    }
                }
                folder.settings -> pluginManager.apply(ReseamSettingsPlugin::class.java)
                else -> {
                    pluginManager.apply(ReseamExtensionPlugin::class.java)
                    if (folder.typed)
                        plugins.withType(ReseamJavascriptPlugin::class.java) {
                            tasks.named("bundleJavascript") {
                                dependsOn(":typecheck-${folder.name}")
                            }
                        }
                    // The engine links the app runtime; extensions only compile against it.
                    plugins.withId("java-library") {
                        dependencies.add(
                            "compileOnly",
                            "app.reseam:reseam-app-runtime:$reseamVersion",
                        )
                        // An extension reads its own folder's settings and those of the extensions
                        // it uses.
                        folder.settings?.let {
                            dependencies.add(ModuleVariant.SETTINGS.dependencies, project(it))
                        }
                        listOf("compileOnly", "implementation").forEach { scope ->
                            configurations
                                .getByName(scope)
                                .dependencies
                                .withType(ProjectDependency::class.java)
                                .configureEach {
                                    val used = folders.firstOrNull { this.path in it.paths() }
                                    if (used != null && used !== folder)
                                        used.settings?.let {
                                            dependencies.add(
                                                ModuleVariant.SETTINGS.dependencies,
                                                project(it),
                                            )
                                        }
                                }
                        }
                    }
                }
            }
        }
    }

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
