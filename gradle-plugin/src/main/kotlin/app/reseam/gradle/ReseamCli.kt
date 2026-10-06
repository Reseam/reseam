// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Project
import org.gradle.api.provider.Provider

internal fun Project.reseamBinary(): Provider<String> =
    providers
        .environmentVariable("RESEAM_BIN")
        .orElse(providers.gradleProperty("reseam.bin"))
        .orElse(
            providers
                .environmentVariable("RESEAM_WORKSPACE")
                .orElse(providers.gradleProperty("reseam.workspace"))
                .map { "$it/target/release/reseam" }
        )
        .orElse("reseam")

internal fun Project.reseamExecutable(): Provider<java.io.File> =
    reseamBinary().zip(providers.environmentVariable("PATH").orElse("")) { binary, path ->
        val candidates =
            if (binary.contains('/') || binary.contains('\\')) {
                listOf(file(binary), file("$binary.exe"))
            } else {
                path.split(java.io.File.pathSeparator).filter(String::isNotEmpty).flatMap {
                    listOf(java.io.File(it, binary), java.io.File(it, "$binary.exe"))
                }
            }
        candidates.firstOrNull { it.isFile }
            ?: throw org.gradle.api.GradleException("Reseam CLI not found: $binary")
    }
