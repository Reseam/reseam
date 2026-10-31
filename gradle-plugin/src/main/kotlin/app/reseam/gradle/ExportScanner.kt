// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import org.objectweb.asm.AnnotationVisitor
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.FieldVisitor
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes

internal class ExportScanner : ClassVisitor(Opcodes.ASM9) {
    lateinit var name: String
    var exported = false
    val members = mutableListOf<ExportedMember>()

    fun scan(file: File): ExportScanner = apply {
        ClassReader(file.readBytes()).accept(this, ClassReader.SKIP_CODE or ClassReader.SKIP_FRAMES)
    }

    override fun visit(
        version: Int,
        access: Int,
        name: String,
        signature: String?,
        superName: String?,
        interfaces: Array<out String>?,
    ) {
        this.name = name
    }

    override fun visitAnnotation(descriptor: String, visible: Boolean): AnnotationVisitor? {
        if (descriptor == EXPORT) exported = true
        return null
    }

    /** The binding's name: `@Export(name = …)` when given, as overloads need. */
    private fun exportName(member: String, found: (String) -> Unit): AnnotationVisitor =
        object : AnnotationVisitor(api) {
            private var name = member

            override fun visit(key: String, value: Any) {
                if (key == "name" && (value as String).isNotEmpty()) name = value
            }

            override fun visitEnd() = found(name)
        }

    override fun visitField(
        access: Int,
        name: String,
        descriptor: String,
        signature: String?,
        value: Any?,
    ): FieldVisitor {
        return object : FieldVisitor(api) {
            override fun visitAnnotation(annotation: String, visible: Boolean): AnnotationVisitor? =
                if (annotation != EXPORT) null
                else
                    exportName(name) {
                        members += ExportedMember(it, name, ExportKind.FIELD, descriptor)
                    }
        }
    }

    override fun visitMethod(
        access: Int,
        name: String,
        descriptor: String,
        signature: String?,
        exceptions: Array<out String>?,
    ): MethodVisitor {
        val kind = if (access and Opcodes.ACC_STATIC != 0) ExportKind.STATIC else ExportKind.METHOD
        // Bridges the compiler adds for an override carry its annotations too.
        if (access and (Opcodes.ACC_BRIDGE or Opcodes.ACC_SYNTHETIC) != 0)
            return object : MethodVisitor(api) {}
        return object : MethodVisitor(api) {
            override fun visitAnnotation(annotation: String, visible: Boolean): AnnotationVisitor? {
                if (annotation != EXPORT) return null
                if (name == "<init>") {
                    exported = true
                    return null
                }
                return exportName(name) { members += ExportedMember(it, name, kind, descriptor) }
            }
        }
    }
}
