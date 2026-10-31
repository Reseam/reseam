// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import kotlin.metadata.KmFunction
import kotlin.metadata.KmProperty
import kotlin.metadata.Visibility
import kotlin.metadata.jvm.KotlinClassMetadata
import kotlin.metadata.jvm.fieldSignature
import kotlin.metadata.jvm.getterSignature
import kotlin.metadata.jvm.setterSignature
import kotlin.metadata.jvm.signature
import kotlin.metadata.visibility
import org.objectweb.asm.AnnotationVisitor
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.FieldVisitor
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes

/** The public signatures of one class, minus what Kotlin metadata marks `internal` or `private`. */
internal class ApiScanner : ClassVisitor(Opcodes.ASM9) {
    val problems = mutableListOf<String>()
    var className = ""
    var visible = false
    private var hidden = emptySet<String>()

    /** Kotlin's own visibility, which `internal` declarations lose in the class file. */
    override fun visitAnnotation(descriptor: String, visible: Boolean): AnnotationVisitor? {
        if (descriptor != "Lkotlin/Metadata;") return null
        return MetadataReader { metadata ->
            when (metadata) {
                is KotlinClassMetadata.Class -> {
                    val km = metadata.kmClass
                    if (!km.visibility.exposed) this.visible = false
                    hidden =
                        hiddenIn(km.functions, km.properties) +
                            km.constructors
                                .filterNot { it.visibility.exposed }
                                .mapNotNull { it.signature?.toString() }
                }
                is KotlinClassMetadata.FileFacade ->
                    hidden = hiddenIn(metadata.kmPackage.functions, metadata.kmPackage.properties)
                is KotlinClassMetadata.MultiFileClassPart ->
                    hidden = hiddenIn(metadata.kmPackage.functions, metadata.kmPackage.properties)
                else -> Unit
            }
        }
    }

    private fun hiddenIn(
        functions: List<KmFunction>,
        properties: List<KmProperty>,
    ): Set<String> =
        (functions.filterNot { it.visibility.exposed }.mapNotNull { it.signature?.toString() } +
                properties.filterNot { it.visibility.exposed }.flatMap(::signatures))
            .toSet()

    private fun signatures(property: KmProperty): List<String> =
        listOfNotNull(
                property.getterSignature,
                property.setterSignature,
                property.fieldSignature,
            )
            .map { it.toString() }

    override fun visit(
        version: Int,
        access: Int,
        name: String,
        signature: String?,
        superName: String?,
        interfaces: Array<out String>?,
    ) {
        className = name
        visible = access and Opcodes.ACC_PUBLIC != 0 && access and Opcodes.ACC_SYNTHETIC == 0
        if (visible) check(name, listOfNotNull(signature, superName) + interfaces.orEmpty())
    }

    override fun visitField(
        access: Int,
        name: String,
        descriptor: String,
        signature: String?,
        value: Any?,
    ): FieldVisitor? {
        if (exposed(access) && "$name:$descriptor" !in hidden)
            check("$className.$name", listOfNotNull(descriptor, signature))
        return null
    }

    override fun visitMethod(
        access: Int,
        name: String,
        descriptor: String,
        signature: String?,
        exceptions: Array<out String>?,
    ): MethodVisitor? {
        if (exposed(access) && "$name$descriptor" !in hidden)
            check("$className.$name$descriptor", listOfNotNull(descriptor, signature))
        return null
    }

    private fun exposed(access: Int) =
        visible &&
            access and (Opcodes.ACC_PUBLIC or Opcodes.ACC_PROTECTED) != 0 &&
            access and Opcodes.ACC_SYNTHETIC == 0

    private fun check(member: String, types: List<String>) {
        if (types.any { "kotlin/" in it }) problems += member
    }
}

/** Collects the `kotlin.Metadata` annotation's values. */
private class MetadataReader(private val read: (KotlinClassMetadata) -> Unit) :
    AnnotationVisitor(Opcodes.ASM9) {
    private val values = mutableMapOf<String, Any>()

    override fun visit(name: String, value: Any) {
        values[name] = value
    }

    override fun visitArray(name: String): AnnotationVisitor =
        object : AnnotationVisitor(api) {
            val items = mutableListOf<Any>()

            override fun visit(ignored: String?, value: Any) {
                items += value
            }

            override fun visitEnd() {
                values[name] = items.toList()
            }
        }

    @Suppress("UNCHECKED_CAST")
    override fun visitEnd() {
        val strings = { key: String ->
            (values[key] as List<String>?)?.toTypedArray() ?: emptyArray()
        }
        read(
            KotlinClassMetadata.readLenient(
                Metadata(
                    kind = values["k"] as Int,
                    metadataVersion = values["mv"] as IntArray,
                    data1 = strings("d1"),
                    data2 = strings("d2"),
                    extraString = values["xs"] as String? ?: "",
                    packageName = values["pn"] as String? ?: "",
                    extraInt = values["xi"] as Int? ?: 0,
                )
            )
        )
    }
}

private val Visibility.exposed
    get() = this == Visibility.PUBLIC || this == Visibility.PROTECTED
