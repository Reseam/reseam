// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.inspect

import app.reseam.patch.PatchRuntime
import app.reseam.patch.Target
import app.reseam.patch.native.reportOutput
import app.reseam.patch.native.reportTarget
import app.reseam.patch.run.ActiveRuntime
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.PrintStream
import java.lang.reflect.InvocationTargetException
import java.lang.reflect.Method
import java.net.URLClassLoader

/**
 * The engine's entry for running Kotlin it compiled against an APK: a target expression to inspect,
 * or a snippet to run. The engine writes the source; the class it compiles to is [SNIPPET], whose
 * static `target()` returns a [Target] or whose static `run(PatchRuntime)` runs the snippet.
 */
internal object Inspector {
    private const val SNIPPET = "app.reseam.inspection.Snippet"
    private const val COMPILER = "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler"

    private var compiler: Pair<List<String>, Any>? = null

    /**
     * Compiles [source] against [classpath] into [output] with the Kotlin compiler in
     * [compilerJars], loaded apart from the patch runtime so its own standard library is the one it
     * sees. Returns the compiler's messages; empty when it compiled without errors.
     */
    @JvmStatic
    @JvmName("compile")
    fun compile(
        compilerJars: Array<String>,
        classpath: String,
        source: String,
        output: String,
    ): String {
        val compiler = compiler(compilerJars.toList())
        val file = File(output, "Snippet.kt").apply { parentFile.mkdirs() }
        file.writeText(source)
        val messages = ByteArrayOutputStream()
        val exec =
            compiler.javaClass.getMethod(
                "exec",
                PrintStream::class.java,
                Array<String>::class.java,
            )
        val arguments =
            arrayOf(
                "-classpath",
                classpath,
                "-d",
                output,
                "-no-stdlib",
                "-no-reflect",
                "-jvm-target",
                "17",
                "-nowarn",
                file.path,
            )
        val code =
            PrintStream(messages, true, Charsets.UTF_8).use { exec.invoke(compiler, it, arguments) }
        return if (code.toString() == "OK") "" else messages.toString(Charsets.UTF_8)
    }

    private fun compiler(jars: List<String>): Any =
        compiler?.takeIf { it.first == jars }?.second
            ?: URLClassLoader(
                    jars.map { File(it).toURI().toURL() }.toTypedArray(),
                    ClassLoader.getPlatformClassLoader(),
                )
                .loadClass(COMPILER)
                .getDeclaredConstructor()
                .newInstance()
                .also { compiler = jars to it }

    /** Resolves the target the snippet compiled into [output] declares, and reports it. */
    @JvmStatic
    @JvmName("inspectTarget")
    fun inspectTarget(runtime: PatchRuntime, output: String) {
        val inspection =
            withSnippet(output) { snippet ->
                val target = invoke(snippet.getMethod("target")) as Target<*>
                ActiveRuntime.run(runtime) { target.inspect(runtime) }
            }
        reportTarget(
            inspection.matches,
            inspection.winner,
            inspection.many,
            inspection.considered.toUInt(),
            inspection.reasons,
            inspection.nearMisses,
            inspection.failure,
        )
    }

    /** Runs the snippet compiled into [output] and reports what it printed. */
    @JvmStatic
    @JvmName("runSnippet")
    fun runSnippet(runtime: PatchRuntime, output: String) {
        val printed = ByteArrayOutputStream()
        val previous = System.out
        System.setOut(PrintStream(printed, true, Charsets.UTF_8))
        try {
            withSnippet(output) { snippet ->
                val run = snippet.getMethod("run", PatchRuntime::class.java)
                ActiveRuntime.run(runtime) { invoke(run, runtime) }
            }
        } finally {
            System.setOut(previous)
            reportOutput(printed.toString(Charsets.UTF_8))
        }
    }

    /** Runs [block] with the snippet class compiled into [output], then closes its loader. */
    private fun <T> withSnippet(output: String, block: (Class<*>) -> T): T =
        URLClassLoader(arrayOf(File(output).toURI().toURL()), Inspector::class.java.classLoader)
            .use { loader -> block(loader.loadClass(SNIPPET)) }

    /**
     * Calls static [method], rethrowing what the snippet threw rather than the reflection wrapper.
     */
    private fun invoke(method: Method, vararg arguments: Any?): Any? =
        try {
            method.invoke(null, *arguments)
        } catch (error: InvocationTargetException) {
            throw error.targetException
        }

    /** Starts accounting for the targets created from now on, before the checked bundles load. */
    @JvmStatic @JvmName("startTargetCheck") fun startTargetCheck() = TargetRegistry.start()

    /** Ends the check: the labels of targets created since it started that no patch resolved. */
    @JvmStatic
    @JvmName("finishTargetCheck")
    fun finishTargetCheck(): Array<String> = TargetRegistry.finish().toTypedArray()
}
