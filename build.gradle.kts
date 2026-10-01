// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

import org.gradle.api.credentials.HttpHeaderCredentials
import org.gradle.authentication.http.HttpHeaderAuthentication

plugins {
    alias(libs.plugins.kotlin.sam.with.receiver) apply false
    alias(libs.plugins.kotlin.serialization) apply false
    alias(libs.plugins.kotlin.jvm) apply false
    alias(libs.plugins.kotlin.multiplatform) apply false
    alias(libs.plugins.android.kmp.library) apply false
    alias(libs.plugins.spotless)
}

spotless {
    kotlin {
        target("**/*.kt")
        targetExclude("**/build/**", "**/generated/**")
        ktfmt(libs.versions.ktfmt.get()).kotlinlangStyle()
    }
    kotlinGradle {
        target("**/*.gradle.kts")
        targetExclude("**/build/**")
        ktfmt(libs.versions.ktfmt.get()).kotlinlangStyle()
    }
}

val sdkVersion =
    providers.gradleProperty("reseamSdkVersion").orElse("0.0.0-local").get().removePrefix("v")

subprojects {
    group = "app.reseam"
    version = sdkVersion

    plugins.withId("maven-publish") {
        configure<PublishingExtension> {
            repositories {
                maven {
                    name = "Forgejo"
                    url = uri("https://git.reseam.app/api/packages/reseam/maven")
                    credentials(HttpHeaderCredentials::class) {
                        name = "Authorization"
                        value =
                            providers
                                .environmentVariable("FORGEJO_PACKAGES_TOKEN")
                                .map { "token $it" }
                                .orElse("")
                                .get()
                    }
                    authentication { create<HttpHeaderAuthentication>("header") }
                }
            }
        }
    }
}
