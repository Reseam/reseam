// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import org.gradle.api.GradleException
import org.objectweb.asm.AnnotationVisitor
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.Label
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type

private const val REACT_MODULE = "Lcom/facebook/react/module/annotations/ReactModule;"
private const val REACT_METHOD = "Lcom/facebook/react/bridge/ReactMethod;"
private const val NULLABLE = "Lorg/jetbrains/annotations/Nullable;"

internal class NativeParameter(val name: String, val type: Type, val nullable: Boolean)

internal class NativeMethod(
    val name: String,
    val parameters: List<NativeParameter>,
    val returnType: Type,
    val nullableReturn: Boolean,
    val synchronous: Boolean,
)

/** A React Native module class: its `@ReactModule` name and its `@ReactMethod`s. */
internal class NativeModuleScanner : ClassVisitor(Opcodes.ASM9) {
    lateinit var className: String
    var moduleName: String? = null
    val methods = mutableListOf<NativeMethod>()

    fun scan(file: File): NativeModuleScanner = apply {
        ClassReader(file.readBytes()).accept(this, ClassReader.SKIP_FRAMES)
    }

    override fun visit(
        version: Int,
        access: Int,
        name: String,
        signature: String?,
        superName: String?,
        interfaces: Array<out String>?,
    ) {
        className = name.replace('/', '.')
    }

    override fun visitAnnotation(descriptor: String, visible: Boolean): AnnotationVisitor? =
        if (descriptor != REACT_MODULE) null
        else
            object : AnnotationVisitor(api) {
                override fun visit(key: String, value: Any) {
                    if (key == "name") moduleName = value as String
                }
            }

    override fun visitMethod(
        access: Int,
        name: String,
        descriptor: String,
        signature: String?,
        exceptions: Array<out String>?,
    ): MethodVisitor? {
        if (access and (Opcodes.ACC_BRIDGE or Opcodes.ACC_SYNTHETIC) != 0) return null
        val types = Type.getArgumentTypes(descriptor)
        val slots =
            types.runningFold(if (access and Opcodes.ACC_STATIC != 0) 0 else 1) { slot, type ->
                slot + type.size
            }
        return object : MethodVisitor(api) {
            private var react = false
            private var synchronous = false
            private var nullableReturn = false
            private val nullable = mutableSetOf<Int>()
            private val names = mutableMapOf<Int, String>()

            override fun visitAnnotation(annotation: String, visible: Boolean): AnnotationVisitor? {
                if (annotation == NULLABLE) nullableReturn = true
                if (annotation != REACT_METHOD) return null
                react = true
                return object : AnnotationVisitor(api) {
                    override fun visit(key: String, value: Any) {
                        if (key == "isBlockingSynchronousMethod") synchronous = value as Boolean
                    }
                }
            }

            override fun visitParameterAnnotation(
                parameter: Int,
                annotation: String,
                visible: Boolean,
            ): AnnotationVisitor? {
                if (annotation == NULLABLE) nullable += parameter
                return null
            }

            override fun visitLocalVariable(
                local: String,
                localDescriptor: String,
                localSignature: String?,
                start: Label,
                end: Label,
                index: Int,
            ) {
                names.putIfAbsent(index, local)
            }

            override fun visitEnd() {
                if (!react) return
                methods +=
                    NativeMethod(
                        name,
                        types.mapIndexed { i, type ->
                            NativeParameter(
                                names[slots[i]]
                                    ?: throw GradleException(
                                        "$className.$name has no name for parameter $i; compile with debug info"
                                    ),
                                type,
                                i in nullable,
                            )
                        },
                        Type.getReturnType(descriptor),
                        nullableReturn,
                        synchronous,
                    )
            }
        }
    }
}
