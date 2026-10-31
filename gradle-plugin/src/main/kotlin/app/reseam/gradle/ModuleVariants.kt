// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Project
import org.gradle.api.attributes.Usage
import org.gradle.api.file.FileCollection
import org.gradle.api.file.RegularFile
import org.gradle.api.provider.Provider
import org.gradle.api.tasks.SourceSet

/** Files one module hands another, outside Java's own variants. */
internal enum class ModuleVariant(val usage: String, val dependencies: String) {
    /** An extension's classes, from which a patch module generates its `@Export` bindings. */
    EXPORTS("reseam-export-classes", "reseamExtensions"),

    /** A settings module's generated extension-side classes, which extensions compile against. */
    SETTINGS("reseam-settings-classes", "reseamSettings"),

    /** A JavaScript extension's exports, from which a patch module generates its bindings. */
    JAVASCRIPT("reseam-javascript-exports", "reseamJavascript"),
}

internal fun Project.publish(variant: ModuleVariant, sourceSet: SourceSet) {
    configurations.consumable(variant.usage) {
        attributes.attribute(Usage.USAGE_ATTRIBUTE, objects.named(Usage::class.java, variant.usage))
        outgoing.artifacts(provider { sourceSet.output.classesDirs.files }) {
            builtBy(sourceSet.output)
        }
    }
}

internal fun Project.publish(variant: ModuleVariant, file: Provider<RegularFile>) {
    configurations.consumable(variant.usage) {
        attributes.attribute(Usage.USAGE_ATTRIBUTE, objects.named(Usage::class.java, variant.usage))
        outgoing.artifact(file)
    }
}

/**
 * The files of the modules added to [ModuleVariant.dependencies], such as
 * `reseamSettings(project(":settings"))`.
 */
internal fun Project.resolve(variant: ModuleVariant): FileCollection {
    val scope = configurations.dependencyScope(variant.dependencies)
    return configurations
        .resolvable("${variant.dependencies}Classpath") {
            extendsFrom(scope.get())
            attributes.attribute(
                Usage.USAGE_ATTRIBUTE,
                objects.named(Usage::class.java, variant.usage),
            )
        }
        .get()
        .incoming
        .files
}
