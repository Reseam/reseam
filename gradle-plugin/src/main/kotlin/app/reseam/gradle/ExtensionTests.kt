// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Project
import org.gradle.api.attributes.Usage
import org.gradle.api.file.FileCollection
import org.gradle.api.tasks.JavaExec
import org.gradle.api.tasks.SourceSet

internal fun Project.configureExtensionTests(
    test: SourceSet,
    appClasses: FileCollection,
    settings: FileCollection,
) {
    configurations
        .getByName(test.compileOnlyConfigurationName)
        .extendsFrom(configurations.getByName("compileOnly"))
    dependencies.add(
        test.runtimeOnlyConfigurationName,
        "app.reseam:reseam-app-runtime:$reseamVersion",
    )
    test.compileClasspath += appClasses + settings

    val android =
        tasks.register("mockableAndroidJar", MockableAndroidJarTask::class.java) {
            platformJars.from(
                provider { listOfNotNull(AndroidSdk.platformJar(), AndroidSdk.testMockJar()) }
            )
            output.set(layout.buildDirectory.file("reseam/test/android.jar"))
        }
    val schema = if (inBundle()) testSettingsSchema() else files()
    // Last, so the libraries a test adds, such as org.json, come before the platform's stubs of
    // them.
    test.runtimeClasspath += appClasses + settings + schema + files(android.flatMap { it.output })
}

private fun Project.testSettingsSchema(): FileCollection {
    val groups = resolve(ModuleVariant.SETTINGS_GROUPS)
    val sdk = configurations.dependencyScope("reseamTestPatchSdk")
    dependencies.add(sdk.name, "app.reseam:reseam-patch-sdk:$reseamVersion")
    val patchSdk =
        configurations.resolvable("reseamTestPatchSdkClasspath") {
            extendsFrom(sdk.get())
            attributes.attribute(
                Usage.USAGE_ATTRIBUTE,
                objects.named(Usage::class.java, Usage.JAVA_RUNTIME),
            )
        }
    val output = layout.buildDirectory.dir("reseam/test/settings")
    val bundle = bundleInfo().name
    val generate =
        tasks.register("generateTestSettings", JavaExec::class.java) {
            classpath = files(patchSdk) + groups
            mainClass.set("app.reseam.patch.settings.TestSettingsSchemaKt")
            inputs.files(groups)
            inputs.property("bundle", bundle)
            outputs.dir(output)
            argumentProviders.add {
                listOf(
                    output.get().file("reseam/${bundle.get()}/settings.json").asFile.absolutePath,
                    bundle.get(),
                ) + groups.files.map { it.absolutePath }
            }
        }
    return files(output).builtBy(generate)
}
