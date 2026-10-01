// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.io.InputStream
import java.nio.file.Files
import java.util.Locale
import java.util.zip.ZipFile
import kotlinx.serialization.encodeToString
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.tasks.Classpath
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.TaskAction
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.FieldVisitor
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type

private const val PATCH_TYPE = "app/reseam/patch/ReseamPatch"

private data class Member(
    val owner: String,
    val name: String,
    val type: Type,
    val kind: MemberKind,
)

private enum class ClassKind {
    CLASS,
    INTERFACE,
}

private data class ClassHeader(
    val kind: ClassKind,
    val parents: List<String>,
    val members: List<Member>,
)

internal abstract class PatchIndexTask : DefaultTask() {
    @get:InputFile abstract val jar: RegularFileProperty
    @get:Classpath abstract val libraries: ConfigurableFileCollection
    @get:OutputFile abstract val output: RegularFileProperty

    @TaskAction
    fun run() {
        val classes = linkedMapOf<String, ClassHeader>()
        val own = readJar(jar.get().asFile)
        libraries.files
            .sortedBy { it.path }
            .forEach { library ->
                val headers =
                    if (library.isDirectory) {
                        library
                            .walkTopDown()
                            .onFail { file, error ->
                                throw GradleException(
                                    "cannot read compiled classes in $file",
                                    error,
                                )
                            }
                            .filter { it.isFile && it.extension == "class" }
                            .associate { file ->
                                file.inputStream().use { readClass(it, file.path) }
                            }
                    } else readJar(library)
                headers.forEach { (name, header) -> classes.putIfAbsent(name, header) }
            }
        classes.putAll(own)
        fun isPatch(type: String, visited: Set<String> = emptySet()): Boolean =
            type == PATCH_TYPE ||
                (type !in visited &&
                    classes[type]?.parents.orEmpty().any { isPatch(it, visited + type) })
        fun members(type: String, visited: Set<String> = emptySet()): List<Member> =
            if (type in visited) emptyList()
            else
                classes[type]
                    ?.let {
                        val ownMethods =
                            it.members
                                .filter { member -> member.kind == MemberKind.METHOD }
                                .map { member -> member.name }
                                .toSet()
                        it.members +
                            it.parents.flatMap { parent ->
                                members(parent, visited + type).filter { member ->
                                    member.kind == MemberKind.FIELD ||
                                        (classes[parent]?.kind == ClassKind.CLASS &&
                                            member.name !in ownMethods)
                                }
                            }
                    }
                    .orEmpty()
        val declarations =
            own.keys
                .sorted()
                .filter { !it.contains('$') }
                .flatMap { className ->
                    val pkg = className.substringBeforeLast('/', "").replace('/', '.')
                    members(className)
                        .filter { it.type.sort == Type.OBJECT && isPatch(it.type.internalName) }
                        .map { member ->
                            val property =
                                when (member.kind) {
                                    MemberKind.FIELD -> member.name
                                    MemberKind.METHOD -> propertyName(member.name)
                                }
                            PatchDeclaration(
                                className.replace('/', '.'),
                                member.owner.replace('/', '.'),
                                member.name,
                                member.kind,
                                if (pkg.isEmpty()) property else "$pkg.$property",
                            )
                        }
                }
                .distinct()
        val file = output.get().asFile
        Files.createDirectories(file.parentFile.toPath())
        file.writeText(bundleJson.encodeToString(declarations))
    }

    private fun readJar(file: File): Map<String, ClassHeader> =
        ZipFile(file).use { jar ->
            jar.entries()
                .asSequence()
                .filter {
                    !it.isDirectory &&
                        it.name.endsWith(".class") &&
                        !it.name.startsWith("META-INF/")
                }
                .associate { entry ->
                    jar.getInputStream(entry).use { readClass(it, "${entry.name} in $file") }
                }
        }

    private fun readClass(stream: InputStream, location: String): Pair<String, ClassHeader> {
        val bytes = stream.readNBytes(16 * 1024 * 1024 + 1)
        if (bytes.size > 16 * 1024 * 1024) throw GradleException("class $location is too large")
        val reader = ClassReader(bytes)
        val members = mutableListOf<Member>()
        reader.accept(
            object : ClassVisitor(Opcodes.ASM9) {
                override fun visitField(
                    access: Int,
                    name: String,
                    descriptor: String,
                    signature: String?,
                    value: Any?,
                ): FieldVisitor? {
                    if (
                        access and (Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC) ==
                            (Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC)
                    )
                        members +=
                            Member(
                                reader.className,
                                name,
                                Type.getType(descriptor),
                                MemberKind.FIELD,
                            )
                    return null
                }

                override fun visitMethod(
                    access: Int,
                    name: String,
                    descriptor: String,
                    signature: String?,
                    exceptions: Array<out String>?,
                ): MethodVisitor? {
                    if (
                        access and (Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC) ==
                            (Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC) &&
                            Type.getArgumentTypes(descriptor).isEmpty()
                    )
                        members +=
                            Member(
                                reader.className,
                                name,
                                Type.getReturnType(descriptor),
                                MemberKind.METHOD,
                            )
                    return null
                }
            },
            ClassReader.SKIP_CODE or ClassReader.SKIP_DEBUG or ClassReader.SKIP_FRAMES,
        )
        return reader.className to
            ClassHeader(
                if (reader.access and Opcodes.ACC_INTERFACE != 0) ClassKind.INTERFACE
                else ClassKind.CLASS,
                listOfNotNull(reader.superName) + reader.interfaces,
                members,
            )
    }

    private fun propertyName(name: String): String {
        val rest = name.removePrefix("get")
        if (rest == name || rest.isEmpty() || !Character.isUpperCase(rest.codePointAt(0)))
            return name
        val first = Character.charCount(rest.codePointAt(0))
        return rest.substring(0, first).lowercase(Locale.ROOT) + rest.substring(first)
    }
}
