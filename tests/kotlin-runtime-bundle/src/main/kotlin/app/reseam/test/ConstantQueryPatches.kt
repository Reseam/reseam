// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.*
import app.reseam.patch.dex.literal

private val prefixedRoutes = methods("prefixedRoutes") { stringsStartingWith("reel/") }

val stringPrefixQueries =
    patch("string-prefix-queries") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        execute {
            check(prefixedRoutes.all.map { it.name }.toSet() == setOf("watch", "create"))
            check(
                klass("com.example.Routes")
                    .methods {
                        stringsStartingWith("reel/")
                        name("embedded")
                    }
                    .all
                    .isEmpty()
            )
            check(
                methods {
                    stringsStartingWith("reel/")
                    includeExtensions()
                }
                    .all
                    .size == 3
            )
            check(methods { stringsStartingWith("absent/") }.all.isEmpty())
            check(runCatching { methods { stringsStartingWith("") }.all }.isFailure)
        }
    }

val high16Literals =
    patch("high16-literals") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        execute {
            for ((name, before, after) in
                listOf(
                    Triple("positive", 67108864L, 134217728L),
                    Triple("negative", -65536L, -131072L),
                    Triple("wide", 1L shl 60, Long.MIN_VALUE),
                )) {
                val target = method { literals(before) }
                check(target.name == name)
                check(target.point { literal(before) }.instruction.literal == before)
                check(target.method.replaceAllLiterals(before, after) == 1)
                check(target.point { literal(after) }.instruction.literal == after)
                check(method { literals(after) }.name == name)
            }
        }
    }
