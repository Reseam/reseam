// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.Project
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
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
 * Compiles `src/main/strings` into the JSON tables the app runtime reads, the extension-side object
 * that reads them, and the patch-side table of references.
 */
internal abstract class StringsTask : DefaultTask() {
    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val sources: DirectoryProperty

    @get:Input abstract val objectName: Property<String>

    @get:Input abstract val tableId: Property<String>

    @get:Input abstract val assetDirectory: Property<String>

    @get:OutputDirectory abstract val json: DirectoryProperty

    @get:OutputDirectory abstract val device: DirectoryProperty

    @get:OutputDirectory abstract val host: DirectoryProperty

    @TaskAction
    fun run() {
        val files = readStringFiles(sources.get().asFile)
        val base = files.single { it.locale == DEFAULT_LOCALE }
        val table = json.get().asFile.also(::recreate).resolve("reseam/strings/${tableId.get()}")
        table.mkdirs()
        for (file in files) table.resolve("${file.locale}.json").writeText(encode(file))
        table.resolve("locales").writeText(files.joinToString("\n", postfix = "\n") { it.locale })
        val names = base.strings.keys.map(::kotlinName)
        if (names.toSet().size != names.size || names.any { it in RESERVED_NAMES })
            throw GradleException(
                "${sources.get()}: string names must map to distinct Kotlin names other than $RESERVED_NAMES"
            )
        writeSource(device, deviceObject(base))
        writeSource(host, hostObject(base))
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

    private fun writeSource(directory: DirectoryProperty, source: String) {
        val (pkg, name) =
            objectName.get().let { it.substringBeforeLast('.', "") to it.substringAfterLast('.') }
        val file =
            directory
                .get()
                .asFile
                .also(::recreate)
                .resolve(pkg.replace('.', '/'))
                .resolve("$name.kt")
        file.parentFile.mkdirs()
        file.writeText(source)
    }

    private fun deviceObject(base: StringFile) = kotlinFile {
        line("import app.reseam.runtime.StringTable")
        line()
        block("object ${simpleName()}") {
            line("private val table = StringTable(${kotlinString(assetDirectory.get())})")
            for ((name, value) in base.strings) {
                line()
                val parameters =
                    value.placeholders.joinToString { "${kotlinName(it.id)}: ${it.type.kotlin}" }
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

    private fun hostObject(base: StringFile) = kotlinFile {
        line("import app.reseam.patch.settings.StringTable")
        line()
        val names = base.strings.keys.joinToString(", ") { "\"$it\"" }
        block(
            "object ${simpleName()} : StringTable(${kotlinString(tableId.get())}, setOf($names))"
        ) {
            for (name in base.strings.keys) line("val ${kotlinName(name)} = string(\"$name\")")
        }
    }

    private fun kotlinFile(body: KotlinWriter.() -> Unit): String =
        KotlinWriter()
            .apply {
                line("// Generated by the Reseam Gradle plugin from src/main/strings.")
                objectName.get().substringBeforeLast('.', "").takeIf(String::isNotEmpty)?.let {
                    line("package $it")
                    line()
                }
                body()
            }
            .toString()

    private fun simpleName() = objectName.get().substringAfterLast('.')
}

/** Registers the string table of a module with `src/main/strings`, or returns `null`. */
internal fun Project.registerStrings(): TaskProvider<StringsTask>? {
    val sources = layout.projectDirectory.dir("src/main/strings")
    if (!sources.asFile.isDirectory) return null
    val strings = extensions.create("reseamStrings", ReseamStringsExtension::class.java)
    strings.tableId.convention(strings.objectName.map { it.substringAfterLast('.') })
    if (inBundle()) {
        strings.assetDirectory.convention(
            bundleInfo().name.zip(strings.tableId) { bundle, table ->
                "reseam/$bundle/strings/$table"
            }
        )
    }
    return tasks.register("generateStrings", StringsTask::class.java) {
        this.sources.set(sources)
        objectName.set(strings.objectName)
        tableId.set(strings.tableId)
        assetDirectory.set(strings.assetDirectory)
        json.set(layout.buildDirectory.dir("generated/reseam/strings/json"))
        device.set(layout.buildDirectory.dir("generated/reseam/strings/device"))
        host.set(layout.buildDirectory.dir("generated/reseam/strings/host"))
    }
}
