// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.isSet
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

internal class Access(private val from: String) {
    private val index
        get() = ActiveRuntime.current.index

    fun requireClass(type: String) {
        val owner = type.trimStart('[')
        if (!owner.startsWith("L") || owner == from || samePackage(owner)) return
        val flags = index.classFor(owner)?.info?.accessFlags ?: return
        require(AccessFlags.PUBLIC.isSet(flags)) {
            "${className(owner)} is not public, so ${className(from)} in another package cannot use it"
        }
    }

    fun requireField(ref: FieldRef) {
        requireClass(ref.definingClass)
        val flags =
            index
                .classFor(ref.definingClass)
                ?.fields
                ?.firstOrNull { it.name == ref.name && it.fieldType == ref.fieldType }
                ?.accessFlags ?: return
        requireMember("${ref.definingClass}->${ref.name}", ref.definingClass, flags)
    }

    fun requireMethod(ref: MethodRef) {
        requireClass(ref.definingClass)
        val method = index.methodFor(ref) ?: return
        requireMember(
            "${ref.definingClass}->${ref.name}${ref.proto}",
            method.owner,
            method.info.accessFlags,
        )
    }

    private fun requireMember(name: String, declaringClass: String, flags: UInt) {
        if (declaringClass == from) return
        require(!AccessFlags.PRIVATE.isSet(flags)) {
            "$name is private to ${className(declaringClass)}, so ${className(from)} cannot reach it"
        }
        require(
            samePackage(declaringClass) ||
                AccessFlags.PUBLIC.isSet(flags) ||
                (AccessFlags.PROTECTED.isSet(flags) && isAssignableType(from, declaringClass))
        ) {
            "$name is package-private, so ${className(from)} in another package cannot reach it"
        }
    }

    private fun samePackage(type: String) = packageOf(type) == packageOf(from)

    private fun packageOf(type: String) = type.substringBeforeLast('/', "")
}
