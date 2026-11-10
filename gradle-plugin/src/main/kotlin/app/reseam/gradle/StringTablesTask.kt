// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.gradle.api.Project
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.api.tasks.TaskProvider

/** Where a module's string table is generated and read. */
abstract class ReseamStringsExtension {
    /** The generated object, such as `app.example.settings.ExampleStrings`. */
    abstract val objectName: Property<String>

    abstract val tableId: Property<String>

    /** The table's directory in the app's assets. */
    abstract val assetDirectory: Property<String>
}

/**
 * Compiles `src/main/strings` into the JSON tables that settings hosts copy into apps and the
 * engine copies into bundle catalogs, and the patch-side table of references to the strings a patch
 * can show as they are.
 */
internal abstract class StringTablesTask : StringObjectTask() {
    @get:Input abstract val tableId: Property<String>

    @get:OutputDirectory abstract val json: DirectoryProperty

    @get:OutputDirectory abstract val host: DirectoryProperty

    @TaskAction
    fun run() {
        val files = readStrings()
        val table = json.get().asFile.also(::recreate).resolve("reseam/strings/${tableId.get()}")
        table.mkdirs()
        for (file in files) table.resolve("${file.locale}.json").writeText(encode(file))
        table.resolve("locales").writeText(files.joinToString("\n", postfix = "\n") { it.locale })
        val shown =
            files
                .single { it.locale == DEFAULT_LOCALE }
                .strings
                .filterValues { it is PlainString && it.placeholders.isEmpty() }
                .keys
        writeObject(host) {
            line("import app.reseam.patch.settings.StringTable")
            line()
            val names = shown.joinToString(", ") { "\"$it\"" }
            block(
                "object ${simpleName()} : StringTable(${kotlinString(tableId.get())}, setOf($names))"
            ) {
                for (name in shown) line("val ${kotlinName(name)} = string(\"$name\")")
            }
        }
    }

    private fun encode(file: StringFile): String =
        JsonObject(
                file.strings.mapValues { (_, value) ->
                    when (value) {
                        is PlainString -> JsonPrimitive(value.text)
                        is PluralString ->
                            JsonObject(value.forms.mapValues { JsonPrimitive(it.value) })
                    }
                }
            )
            .toString()
}

/** The string settings of a module with `src/main/strings`, or `null` when it has none. */
internal fun Project.reseamStrings(): ReseamStringsExtension? {
    extensions.findByType(ReseamStringsExtension::class.java)?.let {
        return it
    }
    if (!layout.projectDirectory.dir("src/main/strings").asFile.isDirectory) return null
    val strings = extensions.create("reseamStrings", ReseamStringsExtension::class.java)
    strings.tableId.convention(strings.objectName.map { it.substringAfterLast('.') })
    if (inBundle()) {
        strings.assetDirectory.convention(
            bundleInfo().name.zip(strings.tableId) { bundle, table ->
                "reseam/$bundle/strings/$table"
            }
        )
    }
    return strings
}

internal fun Project.registerStringTables(
    strings: ReseamStringsExtension
): TaskProvider<StringTablesTask> =
    tasks.register("generateStrings", StringTablesTask::class.java) {
        sources.set(layout.projectDirectory.dir("src/main/strings"))
        objectName.set(strings.objectName)
        tableId.set(strings.tableId)
        json.set(layout.buildDirectory.dir("generated/reseam/strings/json"))
        host.set(layout.buildDirectory.dir("generated/reseam/strings/host"))
    }
