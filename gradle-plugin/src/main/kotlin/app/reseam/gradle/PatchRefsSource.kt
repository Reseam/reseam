// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.nio.file.Files
import org.gradle.api.GradleException

/** The patch IDs of one bundle. */
internal data class BundlePatches(val bundle: String, val patches: List<String>)

/** Writes one `PatchRefs.kt` per package of [listing], declaring an `ExternalPatch` per patch. */
internal fun writePatchRefs(outDir: File, listing: BundlePatches) {
    for ((pkg, ids) in listing.patches.groupBy { it.substringBeforeLast('.', "") }) {
        val segments =
            pkg.split('.').filter(String::isNotEmpty) + ids.map { it.substringAfterLast('.') }
        if (
            segments.any {
                it.contains('`') ||
                    it.contains('/') ||
                    it.contains('\\') ||
                    it.any(Char::isISOControl) ||
                    it in setOf(".", "..")
            }
        )
            throw GradleException("invalid Kotlin declaration in ${listing.bundle}")
        val file =
            outDir.resolve(listing.bundle).resolve(pkg.replace('.', '/')).resolve("PatchRefs.kt")
        Files.createDirectories(file.parentFile.toPath())
        file.writeText(
            buildString {
                if (pkg.isNotEmpty())
                    append("package ${pkg.split('.').joinToString(".") { "`$it`" }}\n\n")
                append("import app.reseam.patch.ExternalPatch\n\n")
                for (id in ids) append(
                    "val `${id.substringAfterLast('.')}` = ExternalPatch(${kotlinString(listing.bundle)}, ${kotlinString(id)})\n"
                )
            }
        )
    }
}
