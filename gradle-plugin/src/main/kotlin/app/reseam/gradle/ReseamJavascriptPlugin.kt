// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Plugin
import org.gradle.api.Project

internal class ReseamJavascriptPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        project.reseamArtifact()
        require(!project.file("src/main/java").isDirectory) {
            "JavaScript and Java extensions need separate modules"
        }
        project.repositories.ivy {
            url = project.uri("https://registry.npmjs.org")
            patternLayout { artifact("[organisation]/-/[module]-[revision].[ext]") }
            metadataSources { artifact() }
            content { includeModule("hermes-compiler", "hermes-compiler") }
        }
        project.tasks.register(HermesCompiler.TASK, HermesCompileTask::class.java) {
            group = "build"
            description =
                "Compiles the JavaScript extension to Hermes v${HermesCompiler.BYTECODE_VERSION} bytecode."
            sources.set(project.layout.projectDirectory.dir("src/main/js"))
            artifact.set(project.reseamArtifact().name)
            output.set(
                project.layout.buildDirectory.dir(
                    "reseam/hermes/v${HermesCompiler.BYTECODE_VERSION}"
                )
            )
        }
    }
}
