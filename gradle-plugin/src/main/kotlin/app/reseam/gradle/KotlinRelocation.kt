// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import org.objectweb.asm.AnnotationVisitor
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.FieldVisitor
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import org.objectweb.asm.TypePath
import org.objectweb.asm.commons.ClassRemapper
import org.objectweb.asm.commons.Remapper

/**
 * Moves the Kotlin standard library under [kotlinPackage]. The app may ship its own, trimmed copy,
 * and each bundle and the app runtime ship theirs, so none of them can use `kotlin.*` itself.
 * [appClasses] are the app's own, such as a `kotlin.jvm.functions.Function0` an extension
 * implements for the app, and keep their names.
 */
internal class KotlinRelocation(
    kotlinPackage: String,
    private val appClasses: Set<String> = emptySet(),
) {
    private val target = kotlinPackage.replace('.', '/') + "/"

    private val remapper =
        object : Remapper(Opcodes.ASM9) {
            override fun map(internalName: String): String =
                if (internalName.startsWith(KOTLIN) && internalName !in appClasses)
                    target + internalName.removePrefix(KOTLIN)
                else internalName
        }

    /** Writes the classes of [inputs], directories or jars, relocated into [output]. */
    fun relocate(inputs: Iterable<File>, output: File) {
        for (input in inputs.filter { it.isDirectory || it.isFile && it.extension == "jar" }) {
            libraryClasses(input) { classes ->
                classes
                    .filter { isClass(it.path) }
                    .forEach { entry -> write(entry.read { it.readBytes() }, output) }
            }
        }
    }

    private fun write(bytes: ByteArray, output: File) {
        val reader = ClassReader(bytes)
        val writer = ClassWriter(0)
        reader.accept(WithoutKotlinCompilerData(ClassRemapper(writer, remapper)), 0)
        val file = output.resolve(remapper.map(reader.className) + ".class")
        file.parentFile.mkdirs()
        file.writeBytes(writer.toByteArray())
    }

    private fun isClass(name: String) =
        name.endsWith(".class") && !name.startsWith("META-INF/") && name != "module-info.class"

    private companion object {
        const val KOTLIN = "kotlin/"
    }
}

/**
 * Drops what only the Kotlin compiler reads: its annotations, and the `InlineMarker` calls it
 * leaves in the bodies of inline functions. Without R8 those calls reach the DEX, and one between a
 * `monitor-enter` and its `monitor-exit` outside a catch-all makes ART reject the whole class.
 */
private class WithoutKotlinCompilerData(next: ClassVisitor) : ClassVisitor(Opcodes.ASM9, next) {
    override fun visitAnnotation(descriptor: String, visible: Boolean): AnnotationVisitor? =
        if (isKotlin(descriptor)) null else super.visitAnnotation(descriptor, visible)

    override fun visitTypeAnnotation(
        typeRef: Int,
        typePath: TypePath?,
        descriptor: String,
        visible: Boolean,
    ): AnnotationVisitor? =
        if (isKotlin(descriptor)) null
        else super.visitTypeAnnotation(typeRef, typePath, descriptor, visible)

    override fun visitField(
        access: Int,
        name: String,
        descriptor: String,
        signature: String?,
        value: Any?,
    ): FieldVisitor =
        object : FieldVisitor(api, super.visitField(access, name, descriptor, signature, value)) {
            override fun visitAnnotation(descriptor: String, visible: Boolean): AnnotationVisitor? =
                if (isKotlin(descriptor)) null else super.visitAnnotation(descriptor, visible)
        }

    override fun visitMethod(
        access: Int,
        name: String,
        descriptor: String,
        signature: String?,
        exceptions: Array<out String>?,
    ): MethodVisitor =
        object :
            MethodVisitor(api, super.visitMethod(access, name, descriptor, signature, exceptions)) {
            override fun visitAnnotation(descriptor: String, visible: Boolean): AnnotationVisitor? =
                if (isKotlin(descriptor)) null else super.visitAnnotation(descriptor, visible)

            override fun visitParameterAnnotation(
                parameter: Int,
                descriptor: String,
                visible: Boolean,
            ): AnnotationVisitor? =
                if (isKotlin(descriptor)) null
                else super.visitParameterAnnotation(parameter, descriptor, visible)

            override fun visitMethodInsn(
                opcode: Int,
                owner: String,
                name: String,
                descriptor: String,
                isInterface: Boolean,
            ) {
                if (owner != INLINE_MARKER)
                    return super.visitMethodInsn(opcode, owner, name, descriptor, isInterface)
                repeat(Type.getArgumentTypes(descriptor).size) { super.visitInsn(Opcodes.POP) }
            }
        }

    private fun isKotlin(descriptor: String) = descriptor.startsWith("Lkotlin/")

    private companion object {
        const val INLINE_MARKER = "kotlin/jvm/internal/InlineMarker"
    }
}
