// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

import org.gradle.api.credentials.HttpHeaderCredentials
import org.gradle.authentication.http.HttpHeaderAuthentication

plugins {
    kotlin("jvm") version "1.9.25" apply false
    kotlin("multiplatform") version "1.9.25" apply false
    id("com.android.library") version "8.11.2" apply false
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
