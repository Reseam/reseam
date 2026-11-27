// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.lang.reflect.Modifier
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Opcodes

class MockableAndroidJarTest {
    @Test
    fun `platform stubs become classes whose methods return defaults`() {
        val type = load(mockable(platformStub()))
        val stub = type.getConstructor(String::class.java).newInstance("ignored")

        assertFalse(Modifier.isFinal(type.modifiers))
        assertEquals(0, type.getMethod("count").invoke(stub))
        assertNull(type.getMethod("name").invoke(stub))
        assertEquals(0L, type.getMethod("time").invoke(stub))
        assertEquals(false, type.getMethod("flag").invoke(null))
        assertFalse(Modifier.isFinal(type.getMethod("count").modifiers))
    }

    private fun platformStub(): ByteArray {
        val writer = ClassWriter(ClassWriter.COMPUTE_FRAMES or ClassWriter.COMPUTE_MAXS)
        writer.visit(
            Opcodes.V1_8,
            Opcodes.ACC_PUBLIC or Opcodes.ACC_FINAL,
            STUB,
            null,
            "java/lang/Object",
            null,
        )
        fun method(access: Int, name: String, descriptor: String, superCall: Boolean = false) {
            val method = writer.visitMethod(access, name, descriptor, null, null)
            if (access and Opcodes.ACC_NATIVE != 0) return method.visitEnd()
            method.visitCode()
            if (superCall) {
                method.visitVarInsn(Opcodes.ALOAD, 0)
                method.visitMethodInsn(
                    Opcodes.INVOKESPECIAL,
                    "java/lang/Object",
                    "<init>",
                    "()V",
                    false,
                )
            }
            method.visitTypeInsn(Opcodes.NEW, "java/lang/RuntimeException")
            method.visitInsn(Opcodes.DUP)
            method.visitLdcInsn("Stub!")
            method.visitMethodInsn(
                Opcodes.INVOKESPECIAL,
                "java/lang/RuntimeException",
                "<init>",
                "(Ljava/lang/String;)V",
                false,
            )
            method.visitInsn(Opcodes.ATHROW)
            method.visitMaxs(0, 0)
            method.visitEnd()
        }
        method(Opcodes.ACC_PUBLIC, "<init>", "(Ljava/lang/String;)V", superCall = true)
        method(Opcodes.ACC_PUBLIC or Opcodes.ACC_FINAL, "count", "()I")
        method(Opcodes.ACC_PUBLIC, "name", "()Ljava/lang/String;")
        method(Opcodes.ACC_PUBLIC or Opcodes.ACC_NATIVE, "time", "()J")
        method(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC, "flag", "()Z")
        method(Opcodes.ACC_STATIC, "<clinit>", "()V")
        writer.visitEnd()
        return writer.toByteArray()
    }

    private fun load(classFile: ByteArray): Class<*> =
        object : ClassLoader(javaClass.classLoader) {
                fun define(): Class<*> =
                    defineClass(STUB.replace('/', '.'), classFile, 0, classFile.size)
            }
            .define()

    private companion object {
        const val STUB = "android/test/Stub"
    }
}
