// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.dex

import app.reseam.patch.ActiveRuntime
import app.reseam.patch.AnnotationItem
import app.reseam.patch.ClassInfo
import app.reseam.patch.EncodedVal
import app.reseam.patch.FieldInfo
import app.reseam.patch.FieldRef
import app.reseam.patch.NewField
import app.reseam.patch.NewMethod
import app.reseam.patch.addClassAnnotation
import app.reseam.patch.addField
import app.reseam.patch.addFieldAnnotation
import app.reseam.patch.addInterface
import app.reseam.patch.addMethod
import app.reseam.patch.classDirectMethods
import app.reseam.patch.classFields
import app.reseam.patch.classVirtualMethods
import app.reseam.patch.definalClass
import app.reseam.patch.removeClass
import app.reseam.patch.removeField
import app.reseam.patch.setClassAccessFlags
import app.reseam.patch.setFieldAccessFlags
import app.reseam.patch.setStaticFieldValue
import app.reseam.patch.setSuperclass
import app.reseam.patch.superclassChain

/** A class in the app's bytecode, identified by an engine handle valid for the running patch. */
@JvmInline
value class DexClass(val handle: UInt) {
    val info: ClassInfo
        get() = ActiveRuntime.current.classInfo(handle)

    val descriptor: String get() = info.descriptor
    val superclass: String? get() = info.superclass
    val interfaces: List<String> get() = info.interfaces
    val sourceFile: String? get() = info.sourceFile
    val isInterface: Boolean get() = AccessFlags.INTERFACE.isSet(info.accessFlags)

    val methods: List<Method> get() = directMethods + virtualMethods
    val directMethods: List<Method> get() = classDirectMethods(handle).map { Method(it.toUInt()) }
    val virtualMethods: List<Method> get() = classVirtualMethods(handle).map { Method(it.toUInt()) }

    val fields: List<FieldInfo> get() = classFields(handle)
    val staticFields: List<FieldInfo> get() = fields.filter { AccessFlags.STATIC.isSet(it.accessFlags) }
    val instanceFields: List<FieldInfo> get() = fields.filterNot { AccessFlags.STATIC.isSet(it.accessFlags) }

    val superclassChain: List<DexClass> get() = superclassChain(handle).map { DexClass(it.toUInt()) }

    /** The method called `name`, narrowed by `proto` when more than one overload exists. */
    fun method(name: String, proto: String? = null): Method? =
        methods.firstOrNull { it.name == name && (proto == null || it.proto == proto) }

    fun field(name: String): FieldRef? =
        fields.firstOrNull { it.name == name }?.let { FieldRef(it.classDescriptor, it.name, it.fieldType) }

    fun setAccessFlags(flags: Int) = setClassAccessFlags(handle, flags.toUInt())
    fun setSuperclass(superclass: String) = setSuperclass(handle, superclass)
    fun addInterface(descriptor: String) = addInterface(handle, descriptor)
    fun definal() = definalClass(handle)
    fun remove() = removeClass(handle)

    fun addMethod(method: NewMethod): Method = Method(addMethod(handle, method))
    fun addField(field: NewField) = addField(handle, field)
    fun removeField(name: String) = removeField(handle, name)
    fun setFieldAccessFlags(fieldName: String, flags: Int) = setFieldAccessFlags(handle, fieldName, flags.toUInt())
    fun setStaticFieldValue(fieldName: String, value: EncodedVal) = setStaticFieldValue(handle, fieldName, value)

    fun addAnnotation(annotation: AnnotationItem) = addClassAnnotation(handle, annotation)
    fun addFieldAnnotation(fieldName: String, annotation: AnnotationItem) = addFieldAnnotation(handle, fieldName, annotation)
}

val FieldInfo.ref: FieldRef
    get() = FieldRef(classDescriptor, name, fieldType)
