// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Plugin
import org.gradle.api.Project

internal class ReseamJavascriptPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        val artifact = project.reseamArtifact()
        require(!project.file("src/main/java").isDirectory) {
            "JavaScript and Java extensions need separate modules"
        }
        project.repositories.npmRegistry(
            packages = setOf(HermesCompiler.PACKAGE),
            scopes = setOf(Esbuild.SCOPE),
        )
        val bundle =
            project.tasks.register("bundleJavascript", JavascriptBundleTask::class.java) {
                group = "build"
                description = "Bundles the extension's entry module and its imports with esbuild."
                sources.set(project.layout.projectDirectory.dir("src/main/js"))
                this.artifact.set(artifact.name)
                binding.set(
                    project.name.split('-').joinToString("") { part ->
                        part.replaceFirstChar(Char::uppercaseChar)
                    }
                )
                script.set(
                    project.layout.buildDirectory.file(
                        artifact.name.map { "reseam/javascript/$it.js" }
                    )
                )
                exports.set(
                    project.layout.buildDirectory.file(
                        artifact.name.map { "reseam/javascript/$it.exports.json" }
                    )
                )
            }
        project.publish(ModuleVariant.JAVASCRIPT, bundle.flatMap { it.exports })
        project.tasks.register(HermesCompiler.TASK, HermesCompileTask::class.java) {
            group = "build"
            description =
                "Compiles the JavaScript extension to Hermes v${HermesCompiler.BYTECODE_VERSION} bytecode."
            script.set(bundle.flatMap { it.script })
            this.artifact.set(artifact.name)
            output.set(
                project.layout.buildDirectory.dir(
                    "reseam/hermes/v${HermesCompiler.BYTECODE_VERSION}"
                )
            )
        }
    }
}
