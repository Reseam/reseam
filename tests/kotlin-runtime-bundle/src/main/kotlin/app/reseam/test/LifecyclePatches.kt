// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.CompatiblePackage
import app.reseam.patch.ExternalPatch
import app.reseam.patch.appEntry
import app.reseam.patch.invoke
import app.reseam.patch.patch
import app.reseam.patch.replace
import app.reseam.patch.settings.SettingsGroup
import app.reseam.patch.settings.section
import app.reseam.patch.settings.settingsHost
import app.reseam.patch.settings.toggle

val finalizeOwner =
    declaredFixture(TestStrings.finalizeOwner, TestStrings.finalizeOwnerDescription) {
        execute {
            log.info("finalize-owner execute")
        }

        afterDependents {
            manifest.addPermission("android.permission.INTERNET")
        }
    }

val runtimeApi =
    declaredFixture(TestStrings.runtimeApi, TestStrings.runtimeApiDescription) {
        val baseVersion =
            stringOption("baseVersion", title = TestStrings.optionBaseversion, default = "2.0-base")
        val splitVersion =
            stringOption(
                "splitVersion",
                title = TestStrings.optionSplitversion,
                default = "2.0-split",
            )
        val splitText =
            stringOption(
                "splitText",
                title = TestStrings.optionSplittext,
                default = "Split patched",
            )

        execute {
            manifest.setVersionName(options[baseVersion])
            manifest.component("config.test").setVersionName(options[splitVersion])
            resources.setString("split_label", options[splitText])
            files.write("assets/base-marker.txt", "base".encodeToByteArray())
            files
                .component("config.test")
                .write("assets/split-marker.txt", "split".encodeToByteArray())
        }
    }

val dependentRuntime =
    declaredFixture(TestStrings.dependentRuntime, TestStrings.dependentRuntimeDescription) {
        dependsOn(finalizeOwner)

        execute {
            files
                .component("config.test")
                .write("assets/dependent-marker.txt", "dependent".encodeToByteArray())
        }
    }

val requiredOption =
    declaredFixture(TestStrings.requiredOption, TestStrings.requiredOptionDescription) {
        enabledByDefault(false)
        val token = stringOption("token", title = TestStrings.optionToken, required = true)

        execute {
            log.info(options[token])
        }
    }

val internalHelper = patch {
    description(TestStrings.internalHelperDescription)
    compatibleWith("com.example.test")

    execute {
        files.write("assets/internal-marker.txt", "internal".encodeToByteArray())
    }
}

val usesInternal =
    declaredFixture(TestStrings.usesInternal, TestStrings.usesInternalDescription) {
        dependsOn(internalHelper)

        execute {
            log.info("uses-internal execute")
        }
    }

object DuplicateSettings : SettingsGroup(TestStrings) {
    val firstEnabled by toggle(default = true, key = "first.enabled")
    val secondEnabled by toggle(default = true, key = "second.enabled")
}

val duplicateSettings =
    settingsHost("duplicates") {
        compatibleWith("com.example.test")
        install {}
    }

val firstAds =
    declaredFixture(TestStrings.hideAds, null) {
        enabledByDefault(false)
        val marker = stringOption("marker", title = TestStrings.optionMarker, default = "first")
        val label =
            stringOption("label.detail", title = TestStrings.optionLabelDetail, default = "nested")
        settings(
            duplicateSettings,
            section(TestStrings.sectionFirst, DuplicateSettings.firstEnabled),
        )
        execute {
            files.write("assets/first-ads.txt", options[marker].encodeToByteArray())
            files.write("assets/first-label.txt", options[label].encodeToByteArray())
        }
    }

val firstAdsAlias = firstAds

val secondAds =
    declaredFixture(TestStrings.hideAds, null) {
        enabledByDefault(false)
        dependsOn(firstAds)
        val marker = stringOption("marker", title = TestStrings.optionMarker, default = "second")
        settings(
            duplicateSettings,
            section(TestStrings.sectionSecond, DuplicateSettings.secondEnabled),
        )
        execute { files.write("assets/second-ads.txt", options[marker].encodeToByteArray()) }
    }

val otherAds =
    patch(TestStrings.hideAds) {
        compatibleWith("com.example.other")
        enabledByDefault(false)
        val marker = stringOption("marker", title = TestStrings.optionMarker, default = "other")
        execute { files.write("assets/other-ads.txt", options[marker].encodeToByteArray()) }
    }

val otherBundleHelper = ExternalPatch("other-bundle", "app.reseam.other.helper")

val needsOtherBundle =
    declaredFixture(TestStrings.needsOtherBundle, TestStrings.needsOtherBundleDescription) {
        dependsOn(otherBundleHelper)
        execute {}
    }

val installedVersionCode =
    patch(TestStrings.installedVersionCode) {
        compatibleWith("com.example.test"("9.9", 1))
        enabledByDefault(false)
        execute {}
    }

val otherVersionCode =
    patch(TestStrings.otherVersionCode) {
        compatibleWith(CompatiblePackage("com.example.test", versionCodes = listOf(2)))
        enabledByDefault(false)
        execute {}
    }

val universalMarker =
    patch(TestStrings.universalMarker) {
        description(TestStrings.universalMarkerDescription)
        execute { files.write("assets/universal-marker.txt", "universal".encodeToByteArray()) }
    }

val ownedCallbacks = run {
    lateinit var builder: app.reseam.patch.PatchBuilder
    patch(TestStrings.ownedCallbacks) {
            builder = this
            compatibleWith("com.example.test")
            enabledByDefault(false)
            execute { files.write("assets/owned-execute.txt", "original".encodeToByteArray()) }
            afterDependents {
                files.write("assets/owned-finalize.txt", "original".encodeToByteArray())
            }
        }
        .also {
            builder.execute { error("a built patch must own its execution callback") }
            builder.afterDependents { error("a built patch must own its finalization callback") }
        }
}

val appEntryHook =
    fixturePatch(TestStrings.appEntryHook, TestStrings.appEntryHookDescription) {
        appEntry {
            callStatic(
                "com.example.Observer",
                "started",
                "(Landroid/app/Application;)V",
                application,
            )
        }
    }

val unwrapApplication =
    declaredFixture(TestStrings.unwrapApplication, TestStrings.unwrapApplicationDescription) {
        enabledByDefault(false)
        dependsOn(appEntryHook)

        execute {
            val wrapper = manifest.applicationClass!!
            manifest.edit {
                findByTag("application").single()["android:name"] =
                    bytecode
                        .findClass(wrapper)!!
                        .superclass!!
                        .removePrefix("L")
                        .removeSuffix(";")
                        .replace('/', '.')
            }
        }
    }
