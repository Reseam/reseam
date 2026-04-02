package dev.stitch.patch

class DexClass(val handle: UInt) {
    val info: ClassInfo
        get() = getClassInfo(handle)
            ?: error("invalid class handle: $handle")

    val methods: List<Method>
        get() = classMethods(handle).map { Method(it.toUInt()) }

    val directMethods: List<Method>
        get() = classDirectMethods(handle).map { Method(it.toUInt()) }

    val virtualMethods: List<Method>
        get() = classVirtualMethods(handle).map { Method(it.toUInt()) }

    val fields: List<FieldInfo>
        get() = classFields(handle)

    val staticFields: List<FieldInfo>
        get() = classStaticFields(handle)

    val instanceFields: List<FieldInfo>
        get() = classInstanceFields(handle)

    val superclass: String?
        get() = info.superclass

    val sourceFile: String?
        get() = info.sourceFile

    val superclassChain: List<DexClass>
        get() = superclassChain(handle).map { DexClass(it.toUInt()) }

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
    fun addFieldAnnotation(fieldName: String, annotation: AnnotationItem) =
        addFieldAnnotation(handle, fieldName, annotation)
}
