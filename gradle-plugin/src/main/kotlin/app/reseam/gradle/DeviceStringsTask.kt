// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Project
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.api.tasks.TaskProvider

/** Generates the extension-side object that reads a module's string table from the app's assets. */
internal abstract class DeviceStringsTask : StringObjectTask() {
    /** The table's directory in the app's assets. */
    @get:Input abstract val assetDirectory: Property<String>

    @get:OutputDirectory abstract val device: DirectoryProperty

    @TaskAction
    fun run() {
        val base = readStrings().single { it.locale == DEFAULT_LOCALE }
        writeObject(device) {
            line("import app.reseam.runtime.StringTable")
            line()
            block("object ${simpleName()}") {
                line("private val table = StringTable(${kotlinString(assetDirectory.get())})")
                for ((name, value) in base.strings) {
                    line()
                    val parameters =
                        value.placeholders.joinToString {
                            "${kotlinName(it.id)}: ${it.type.kotlin}"
                        }
                    val arguments = value.placeholders.joinToString { kotlinName(it.id) }
                    when {
                        value is PluralString ->
                            line(
                                "fun ${kotlinName(name)}($parameters): String = " +
                                    "table.plural(\"$name\", count, $arguments)"
                            )
                        value.placeholders.isEmpty() ->
                            line("val ${kotlinName(name)}: String get() = table[\"$name\"]")
                        else ->
                            line(
                                "fun ${kotlinName(name)}($parameters): String = " +
                                    "table.format(\"$name\", $arguments)"
                            )
                    }
                }
            }
        }
    }
}

internal fun Project.registerDeviceStrings(
    strings: ReseamStringsExtension
): TaskProvider<DeviceStringsTask> =
    tasks.register("generateDeviceStrings", DeviceStringsTask::class.java) {
        sources.set(layout.projectDirectory.dir("src/main/strings"))
        objectName.set(strings.objectName)
        assetDirectory.set(strings.assetDirectory)
        device.set(layout.buildDirectory.dir("generated/reseam/strings/device"))
    }
