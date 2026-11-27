// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.util.jar.JarFile
import java.util.jar.JarOutputStream
import java.util.zip.ZipEntry
import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.tasks.CacheableTask
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Handle
import org.objectweb.asm.Label
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type

/**
 * Android's platform classes as JVM tests run them. The platform jar only declares the framework:
 * each method throws. Here each one returns its type's default value and nothing is final, as in
 * the jar Android's own unit tests run against with `returnDefaultValues`.
 */
@CacheableTask
internal abstract class MockableAndroidJarTask : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val platformJars: ConfigurableFileCollection

    @get:OutputFile abstract val output: RegularFileProperty

    @TaskAction
    fun run() {
        val written = mutableSetOf<String>()
        JarOutputStream(output.get().asFile.outputStream().buffered()).use { out ->
            for (jar in platformJars.files) {
                JarFile(jar).use { input ->
                    for (entry in input.entries()) {
                        // The JDK provides java.*; a class path may not define it.
                        if (!entry.name.endsWith(".class") || entry.name.startsWith("java/"))
                            continue
                        if (!written.add(entry.name)) continue
                        out.putNextEntry(ZipEntry(entry.name))
                        out.write(mockable(input.getInputStream(entry).readBytes()))
                        out.closeEntry()
                    }
                }
            }
        }
    }
}

internal fun mockable(classFile: ByteArray): ByteArray {
    val writer =
        object : ClassWriter(COMPUTE_FRAMES) {
            // The rewritten code never merges two types, so no class has to be loaded.
            override fun getCommonSuperClass(type1: String, type2: String) = "java/lang/Object"
        }
    ClassReader(classFile).accept(MockableClass(writer), ClassReader.SKIP_FRAMES)
    return writer.toByteArray()
}

private class MockableClass(next: ClassVisitor) : ClassVisitor(Opcodes.ASM9, next) {
    private lateinit var name: String
    private var superName: String? = null

    override fun visit(
        version: Int,
        access: Int,
        name: String,
        signature: String?,
        superName: String?,
        interfaces: Array<out String>?,
    ) {
        this.name = name
        this.superName = superName
        super.visit(
            version,
            access and Opcodes.ACC_FINAL.inv(),
            name,
            signature,
            superName,
            interfaces,
        )
    }

    override fun visitInnerClass(
        name: String,
        outerName: String?,
        innerName: String?,
        access: Int,
    ) = super.visitInnerClass(name, outerName, innerName, access and Opcodes.ACC_FINAL.inv())

    override fun visitMethod(
        access: Int,
        name: String,
        descriptor: String,
        signature: String?,
        exceptions: Array<out String>?,
    ): MethodVisitor? {
        val open = access and (Opcodes.ACC_FINAL or Opcodes.ACC_NATIVE).inv()
        val method = super.visitMethod(open, name, descriptor, signature, exceptions)
        return when {
            access and Opcodes.ACC_ABSTRACT != 0 -> method
            name == "<init>" -> ConstructorBody(method, setOfNotNull(this.name, superName))
            else -> DefaultBody(method, Type.getReturnType(descriptor))
        }
    }
}

private class DefaultBody(private val method: MethodVisitor, private val returnType: Type) :
    MethodVisitor(Opcodes.ASM9) {
    override fun visitEnd() {
        method.visitCode()
        when (returnType.sort) {
            Type.VOID -> method.visitInsn(Opcodes.RETURN)
            Type.LONG -> {
                method.visitInsn(Opcodes.LCONST_0)
                method.visitInsn(Opcodes.LRETURN)
            }
            Type.FLOAT -> {
                method.visitInsn(Opcodes.FCONST_0)
                method.visitInsn(Opcodes.FRETURN)
            }
            Type.DOUBLE -> {
                method.visitInsn(Opcodes.DCONST_0)
                method.visitInsn(Opcodes.DRETURN)
            }
            Type.ARRAY,
            Type.OBJECT -> {
                method.visitInsn(Opcodes.ACONST_NULL)
                method.visitInsn(Opcodes.ARETURN)
            }
            else -> {
                method.visitInsn(Opcodes.ICONST_0)
                method.visitInsn(Opcodes.IRETURN)
            }
        }
        method.visitMaxs(0, 0)
        method.visitEnd()
    }
}

/**
 * Keeps a constructor up to its call of the superclass (or another own) constructor, which the JVM
 * requires, and returns there.
 */
private class ConstructorBody(private val method: MethodVisitor, private val owners: Set<String>) :
    MethodVisitor(Opcodes.ASM9) {
    private var done = false

    override fun visitCode() = method.visitCode()

    override fun visitInsn(opcode: Int) = keep { method.visitInsn(opcode) }

    override fun visitIntInsn(opcode: Int, operand: Int) = keep {
        method.visitIntInsn(opcode, operand)
    }

    override fun visitVarInsn(opcode: Int, varIndex: Int) = keep {
        method.visitVarInsn(opcode, varIndex)
    }

    override fun visitTypeInsn(opcode: Int, type: String) = keep {
        method.visitTypeInsn(opcode, type)
    }

    override fun visitFieldInsn(opcode: Int, owner: String, name: String, descriptor: String) =
        keep {
            method.visitFieldInsn(opcode, owner, name, descriptor)
        }

    override fun visitLdcInsn(value: Any) = keep { method.visitLdcInsn(value) }

    override fun visitLabel(label: Label) = keep { method.visitLabel(label) }

    override fun visitIincInsn(varIndex: Int, increment: Int) = keep {
        method.visitIincInsn(varIndex, increment)
    }

    override fun visitMultiANewArrayInsn(descriptor: String, numDimensions: Int) = keep {
        method.visitMultiANewArrayInsn(descriptor, numDimensions)
    }

    override fun visitJumpInsn(opcode: Int, label: Label) = branch()

    override fun visitTableSwitchInsn(min: Int, max: Int, dflt: Label, vararg labels: Label) =
        branch()

    override fun visitLookupSwitchInsn(dflt: Label, keys: IntArray, labels: Array<out Label>) =
        branch()

    private fun branch() =
        check(done) {
            "A constructor of ${owners.first()} branches before its superclass constructor"
        }

    override fun visitInvokeDynamicInsn(
        name: String,
        descriptor: String,
        bootstrapMethodHandle: Handle,
        vararg bootstrapMethodArguments: Any,
    ) = keep {
        method.visitInvokeDynamicInsn(
            name,
            descriptor,
            bootstrapMethodHandle,
            *bootstrapMethodArguments,
        )
    }

    override fun visitMethodInsn(
        opcode: Int,
        owner: String,
        name: String,
        descriptor: String,
        isInterface: Boolean,
    ) = keep {
        method.visitMethodInsn(opcode, owner, name, descriptor, isInterface)
        if (opcode == Opcodes.INVOKESPECIAL && name == "<init>" && owner in owners) {
            method.visitInsn(Opcodes.RETURN)
            done = true
        }
    }

    override fun visitEnd() {
        check(done) { "A constructor of ${owners.first()} calls no superclass constructor" }
        method.visitMaxs(0, 0)
        method.visitEnd()
    }

    private fun keep(write: () -> Unit) {
        if (!done) write()
    }
}
