// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

import org.gradle.api.credentials.HttpHeaderCredentials
import org.gradle.authentication.http.HttpHeaderAuthentication

plugins {
    kotlin("plugin.sam.with.receiver") version "2.4.10" apply false
    kotlin("jvm") version "2.4.10" apply false
    kotlin("multiplatform") version "2.4.10" apply false
    id("com.android.kotlin.multiplatform.library") version "9.1.1" apply false
}

val sdkVersion = providers.gradleProperty("reseamSdkVersion").orElse("0.0.0-local").get().removePrefix("v")

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
                        value = providers.environmentVariable("FORGEJO_PACKAGES_TOKEN").map { "token $it" }.orElse("").get()
                    }
                    authentication { create<HttpHeaderAuthentication>("header") }
                }
            }
        }
    }
}
