// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

/**
 * Type descriptors for the types patches name most. Every API that takes a
 * type accepts a descriptor (`Ljava/lang/String;`), a dotted class name
 * (`java.lang.String`), or one of these constants.
 */
object Type {
    const val Void = "V"
    const val Boolean = "Z"
    const val Byte = "B"
    const val Short = "S"
    const val Char = "C"
    const val Int = "I"
    const val Long = "J"
    const val Float = "F"
    const val Double = "D"
    const val Object = "Ljava/lang/Object;"
    const val String = "Ljava/lang/String;"
    const val CharSequence = "Ljava/lang/CharSequence;"
    const val List = "Ljava/util/List;"
    const val ArrayList = "Ljava/util/ArrayList;"
    const val Map = "Ljava/util/Map;"
    const val Context = "Landroid/content/Context;"
    const val View = "Landroid/view/View;"
    const val Activity = "Landroid/app/Activity;"
    const val Application = "Landroid/app/Application;"
}

private val primitives = setOf("V", "Z", "B", "S", "C", "I", "J", "F", "D")

/** The DEX descriptor of `type`, which may already be one. */
fun descriptor(type: String): String = when {
    type in primitives -> type
    type.startsWith("[") -> "[" + descriptor(type.substring(1))
    type.endsWith("[]") -> "[" + descriptor(type.removeSuffix("[]"))
    type.startsWith("L") && type.endsWith(";") -> type
    else -> "L${type.replace('.', '/')};"
}

/** The dotted class name of a class descriptor. */
fun className(descriptor: String): String =
    descriptor.removePrefix("L").removeSuffix(";").replace('/', '.')

/** A method prototype descriptor such as `(Ljava/lang/String;Z)V`. */
fun proto(returns: String, vararg params: String): String =
    params.joinToString("", prefix = "(", postfix = ")") { descriptor(it) } + descriptor(returns)
