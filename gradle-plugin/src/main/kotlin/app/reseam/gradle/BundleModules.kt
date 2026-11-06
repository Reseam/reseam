// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Project
import org.gradle.api.artifacts.ProjectDependency
import org.gradle.api.provider.Provider
import org.gradle.api.tasks.bundling.Jar

private const val KOTLIN_MODULE = "kotlin"
private const val GLOBALS_MODULE = "d8-globals"

/**
 * The modules of the bundle this root project builds: each patch, extension and settings project,
 * the shared Kotlin stdlib and d8 globals, and the root's `resources/`. A module uses the projects
 * it depends on; DEX modules also use the stdlib and globals, patch modules the root's resources.
 */
internal fun Project.bundleModules(dex: BundleDex): Provider<List<StagedModule>> = provider {
    val patches = subprojects.filter { it.plugins.hasPlugin(ReseamPatchesPlugin::class.java) }
    val javascript = subprojects.filter { it.plugins.hasPlugin(ReseamJavascriptPlugin::class.java) }
    val devices = dex.modules.get().map { it.get().project }
    val projects = (patches + javascript + devices).toSet()
    val shared = listOf(KOTLIN_MODULE, GLOBALS_MODULE)

    fun module(name: String, uses: List<String>, vararg contents: StagedFiles) =
        objects.newInstance(StagedModule::class.java).apply {
            this.name.set(name)
            this.uses.set(uses)
            this.contents.set(contents.toList())
        }

    fun files(from: Any, configure: StagedFiles.() -> Unit = {}) =
        objects.newInstance(StagedFiles::class.java).apply {
            this.from.from(from)
            configure()
        }

    fun dexFiles(task: Any, name: String) =
        files(task) {
            include.set("classes*.dex")
            dexName.set(name)
        }

    fun uses(project: Project): List<String> {
        val seen = mutableSetOf<Project>()
        fun walk(from: Project): List<Project> =
            from.configurations
                .flatMap { it.dependencies.withType(ProjectDependency::class.java) }
                .map { project(it.path) }
                .filter { seen.add(it) }
                .flatMap { if (it in projects) listOf(it) else walk(it) }
        return walk(project).map { it.path }
    }

    patches.map { project ->
        module(
            project.path,
            uses(project) + path,
            files(project.tasks.named("patchJar", Jar::class.java).flatMap { it.archiveFile }),
        )
    } +
        javascript.map { project ->
            module(
                project.path,
                uses(project),
                files(
                    project.tasks
                        .named(HermesCompiler.TASK, HermesCompileTask::class.java)
                        .flatMap { it.output }
                ) {
                    include.set("*.hbc")
                    into.set(HermesCompiler.BUNDLE_FOLDER)
                },
            )
        } +
        dex.modules.get().map { task ->
            val project = task.get().project
            module(
                project.path,
                uses(project) + shared,
                dexFiles(task.flatMap { it.output }, project.reseamArtifact().name.get()),
            )
        } +
        module(
                KOTLIN_MODULE,
                emptyList(),
                dexFiles(dex.kotlin.flatMap { it.output }, KOTLIN_MODULE),
            )
            .apply {
                source.set(
                    objects.newInstance(ModuleSource::class.java).apply {
                        val shrink = dex.kotlinStdlib.shrunk.get()
                        files.from(
                            dex.kotlinStdlib.relocated.flatMap { it.output },
                            shrink.compiler,
                            shrink.libraries,
                        )
                        values.add(shrink.rules)
                    }
                )
            } +
        module(
                GLOBALS_MODULE,
                emptyList(),
                dexFiles(dex.globals.flatMap { it.output }, GLOBALS_MODULE),
            )
            .apply {
                source.set(
                    objects.newInstance(ModuleSource::class.java).apply {
                        val globals = dex.globals.get()
                        files.from(globals.compiler, globals.libraries)
                    }
                )
            } +
        module(
            path,
            emptyList(),
            files(layout.projectDirectory.dir("resources")) { into.set("resources") },
        )
}
