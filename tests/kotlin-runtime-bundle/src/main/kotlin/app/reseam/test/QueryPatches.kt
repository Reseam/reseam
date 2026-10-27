// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.alwaysReturn
import app.reseam.patch.before
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.opcode
import app.reseam.patch.enumConstant
import app.reseam.patch.field
import app.reseam.patch.invoke
import app.reseam.patch.klass
import app.reseam.patch.method
import app.reseam.patch.methods
import app.reseam.patch.replace

val extensionShadow =
    fixturePatch("extension-shadow") {
        check(bytecode.findClass("app.reseam.test.ext.VideoState") != null)
        check(
            klass("app.reseam.test.ext.VideoState").method("names").owner ==
                "Lapp/reseam/test/ext/VideoState;"
        )
        files.write(
            "assets/extension-shadow.txt",
            "${appVideoState.descriptor}|${anyVideoState.descriptor}".encodeToByteArray(),
        )
    }

val instructionShapeQueries =
    fixturePatch("instruction-shape-queries") {
        check(unorderedShape.all.size == 2) {
            "expected both shapes, got ${unorderedShape.all.map { it.descriptor }}"
        }
        files.write(
            "assets/instruction-shape.txt",
            "${orderedShape.name}|${shortestMethod.name}|${publicFinalHook.name}|${binderClass.descriptor}"
                .encodeToByteArray(),
        )
        val owner = bytecode.findClass("com.example.ShapeHolder")!!
        val changed = orderedShape.method
        check(changed.info.instructionCount > 1u)
        orderedShape.alwaysReturn()
        check(changed.instructionCount == 1)
        check(
            methods {
                inClass(shapeHolder)
                opcode(Opcode.CONST, Opcode.CONST_STRING)
            }
                .all
                .none { it.method.handle == changed.handle }
        )
        val before = owner.info.directMethodCount + owner.info.virtualMethodCount
        val added = changed.clone("newDeclaration")
        check(owner.info.directMethodCount + owner.info.virtualMethodCount == before + 1u)
        check(
            method {
                inClass(shapeHolder)
                name("newDeclaration")
            }
                .method
                .handle == added.handle
        )
        added.remove()
        check(owner.info.directMethodCount + owner.info.virtualMethodCount == before)
        check(
            methods {
                inClass(shapeHolder)
                name("newDeclaration")
            }
                .all
                .isEmpty()
        )
    }

val inheritedMethods =
    fixturePatch("inherited-methods") {
        val declaredOnly = runCatching { preferenceFragment.method("findPreference").method }
        check(declaredOnly.isFailure)
        files.write(
            "assets/inherited-methods.txt",
            "${inheritedFindPreference.owner}|${findPreferenceCallee.owner}".encodeToByteArray(),
        )
    }

private val planEnum = klass("plan") { enumConstants("FREE", "PREMIUM") }

val enumConstants =
    fixturePatch("enum-constants") {
        val premium = planEnum.enumConstant("PREMIUM")
        klass("com.example.Account").method("plan").replace { returnValue(staticField(premium)) }
        files.write(
            "assets/enum-constants.txt",
            "${planEnum.descriptor}|${premium.name}|${planEnum.enumConstant("FAMILY").name}"
                .encodeToByteArray(),
        )
    }

val fieldAccesses =
    fixturePatch("field-accesses") {
        val premium = planEnum.enumConstant("PREMIUM")
        val readers = methods {
            accesses {
                owner(planEnum.descriptor)
                name(premium.name)
            }
        }
            .all
            .map { "${it.owner}.${it.name}" }
            .sorted()
        val read = klass("com.example.Account").method("plan").field { owner(planEnum.descriptor) }
        files.write(
            "assets/field-accesses.txt",
            (readers + "${read.owner}.${read.name}").joinToString("|").encodeToByteArray(),
        )
    }

private val licenseEnum = klass("license") { enumConstants("NO_LICENSE", "VALID") }

private val tierEnum = klass("tier") { enumConstants("BASIC", "PRO") }

val optimizedEnumConstants =
    fixturePatch("optimized-enum-constants") {
        val valid = licenseEnum.enumConstant("VALID")
        val pro = tierEnum.enumConstant("PRO")
        val account = klass("com.example.Account")
        account.method("license").replace { returnValue(staticField(valid)) }
        account.method("tier").replace { returnValue(staticField(pro)) }
        files.write(
            "assets/optimized-enum-constants.txt",
            "${licenseEnum.descriptor}|${valid.name}|${tierEnum.descriptor}|${pro.name}"
                .encodeToByteArray(),
        )
    }
