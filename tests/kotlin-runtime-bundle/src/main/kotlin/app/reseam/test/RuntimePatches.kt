// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.ExtClass
import app.reseam.patch.ExternalPatch
import app.reseam.patch.ResourceValue
import app.reseam.patch.Type
import app.reseam.patch.XmlDocument
import app.reseam.patch.after
import app.reseam.patch.alwaysReturn
import app.reseam.patch.appEntry
import app.reseam.patch.before
import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.opcode
import app.reseam.patch.klass
import app.reseam.patch.method
import app.reseam.patch.methods
import app.reseam.patch.native.FieldRef
import app.reseam.patch.native.NewField
import app.reseam.patch.patch
import app.reseam.patch.point
import app.reseam.patch.replace
import app.reseam.patch.reserveLocal
import app.reseam.patch.settings.ToggleSetting
import app.reseam.patch.settings.section
import app.reseam.patch.settings.settingsHost
import app.reseam.patch.skipWhen

val finalizeOwner =
    declaredFixture("finalize-owner") {
        execute {
            log.info("finalize-owner execute")
        }

        afterDependents {
            manifest.addPermission("android.permission.INTERNET")
        }
    }

val runtimeApi =
    declaredFixture("runtime-api") {
        val baseVersion = stringOption("baseVersion", default = "2.0-base")
        val splitVersion = stringOption("splitVersion", default = "2.0-split")
        val splitText = stringOption("splitText", default = "Split patched")

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
    declaredFixture("dependent-runtime") {
        dependsOn(finalizeOwner)

        execute {
            files
                .component("config.test")
                .write("assets/dependent-marker.txt", "dependent".encodeToByteArray())
        }
    }

val requiredOption =
    declaredFixture("required-option") {
        enabledByDefault(false)
        val token = stringOption("token", required = true)

        execute {
            log.info(options[token])
        }
    }

val internalHelper = patch {
    description("An internal dependency: never listed, runs when something depends on it")
    compatibleWith("com.example.test")

    execute {
        files.write("assets/internal-marker.txt", "internal".encodeToByteArray())
    }
}

val usesInternal =
    declaredFixture("uses-internal") {
        dependsOn(internalHelper)

        execute {
            log.info("uses-internal execute")
        }
    }

val afterEntryValues =
    fixturePatch("after-entry-values") {
        val target = klass("com.example.HookTarget")
        check(target.method("invokeGrowth").method.growLocalRegisters(10))
        target.method("invokeGrowth").after {
            callStatic("com.example.Observer", "entry", "(I)V", param(3))
        }
        val receiver = target.method("receiver").method
        val noRegister = runCatching {
            receiver.findFreeRegister(0, (0 until receiver.registersSize).toList())
        }
        check(noRegister.isFailure)
        target.method("temporaryReuse").after {
            val held = long(0x123456789abcdef0L)
            repeat(32) {
                callStatic("com.example.Observer", "wide", "(J)V", long(it.toLong()))
                callStatic("com.example.Observer", "scalar", "(I)V", int(it))
            }
            whenTrue(param(0)) {
                callStatic("com.example.Observer", "wide", "(J)V", held)
            } otherwise
                {
                    callStatic("com.example.Observer", "wide", "(J)V", held)
                }
            capture("result").assign(held)
        }
        target.method("getFeatureSwitchValue").after {
            val marker = int(42)
            callStatic(
                "com.example.Observer",
                "record",
                "(ILjava/lang/String;JDLjava/lang/String;Ljava/lang/String;Ljava/lang/Object;)V",
                marker,
                param(0),
                paramOfType(Type.Long),
                param(2),
                lastParam,
                paramOfType(Type.String),
                capture("result"),
            )
        }
        target.method("receiver").after {
            callStatic(
                "com.example.Observer",
                "receiver",
                "(Lcom/example/HookTarget;)V",
                thisObject,
            )
        }
        target.method("resultOnly").after {
            capture("result").assign(int(42))
        }
    }

val embeddedStrings =
    fixturePatch("embedded-strings") {
        val changed =
            bytecode.replaceStringsContaining("com.google.android.gsf") { old ->
                old.takeIf { it.startsWith("content://") }
                    ?.replace("com.google.android.gsf", "app.reseam.gsf")
            }
        files.write("assets/embedded-strings.txt", changed.toString().encodeToByteArray())
    }

val duplicateSettings =
    settingsHost("duplicates") {
        compatibleWith("com.example.test")
        install {}
    }

val firstAds =
    declaredFixture("Hide Ads") {
        enabledByDefault(false)
        val marker = stringOption("marker", default = "first")
        val label = stringOption("label.detail", default = "nested")
        settings(
            duplicateSettings,
            section("First", ToggleSetting("first.enabled", "Enabled", default = true)),
        )
        execute {
            files.write("assets/first-ads.txt", options[marker].encodeToByteArray())
            files.write("assets/first-label.txt", options[label].encodeToByteArray())
        }
    }

val firstAdsAlias = firstAds

val secondAds =
    declaredFixture("Hide Ads") {
        enabledByDefault(false)
        dependsOn(firstAds)
        val marker = stringOption("marker", default = "second")
        settings(
            duplicateSettings,
            section("Second", ToggleSetting("second.enabled", "Enabled", default = true)),
        )
        execute { files.write("assets/second-ads.txt", options[marker].encodeToByteArray()) }
    }

val otherAds =
    patch("Hide Ads") {
        compatibleWith("com.example.other")
        enabledByDefault(false)
        val marker = stringOption("marker", default = "other")
        execute { files.write("assets/other-ads.txt", options[marker].encodeToByteArray()) }
    }

val otherBundleHelper = ExternalPatch("other-bundle", "app.reseam.other.helper")

val needsOtherBundle =
    declaredFixture("needs-other-bundle") {
        dependsOn(otherBundleHelper)
        execute {}
    }

val universalMarker =
    patch("universal-marker") {
        description("Declares no package, so it works with any app and waits to be asked for")
        execute { files.write("assets/universal-marker.txt", "universal".encodeToByteArray()) }
    }

val appEntryHook =
    fixturePatch("app-entry-hook") {
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
    declaredFixture("unwrap-application") {
        enabledByDefault(false)
        dependsOn(appEntryHook)

        execute {
            manifest.edit {
                val application = findByTag("application").single()
                application["android:name"] =
                    bytecode
                        .findClass(application["android:name"]!!)!!
                        .superclass!!
                        .removePrefix("L")
                        .removeSuffix(";")
                        .replace('/', '.')
            }
        }
    }

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

val xmlAttributeBinding =
    fixturePatch("xml-attribute-binding") {
        manifest.edit {
            root.appendChild(
                createElement("intent").apply {
                    this["android:targetPackage"] = "com.example.target"
                    this["android:targetClass"] = "com.example.target.Settings"
                }
            )
            val unknown = runCatching { root["android:notAnAttribute"] = "x" }
            check(unknown.isFailure)
            val undeclared = runCatching { root["yt:layout_constraintLeft_toLeftOf"] = "x" }
            check(undeclared.isFailure)
        }
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
        changed.alwaysReturn()
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

val skipWhenCall =
    fixturePatch("skip-when") {
        skipHost
            .method("straight")
            .point { literal(2) }
            .next { invokeStatic { name("mark") } }
            .skipWhen { bool(true) }
        skipHost
            .method("joined")
            .point { literal(2) }
            .next { invokeStatic { name("mark") } }
            .next { invokeStatic { name("mark") } }
            .skipWhen { bool(true) }
        skipHost
            .method("joinedKept")
            .point { literal(2) }
            .next { invokeStatic { name("mark") } }
            .next { invokeStatic { name("mark") } }
            .skipWhen { bool(false) }
    }

val beforeJoin =
    fixturePatch("before-join") {
        skipHost
            .method("joinedBefore")
            .point { literal(2) }
            .next { invokeStatic { name("mark") } }
            .next { invokeStatic { name("mark") } }
            .before { mark(9) }
    }

val whenInstanceOfBranch =
    fixturePatch("when-instance-of") {
        skipHost.method("typed").replace {
            whenInstanceOf(param(0), "java.lang.String") { mark(1) } otherwise { mark(2) }
            returnVoid()
        }
    }

val replaceBothReturn =
    fixturePatch("replace-both-return") {
        anchorHost.method("replaced").replace {
            whenTrue(param(0)) { returnValue(int(10)) } otherwise { returnValue(int(20)) }
        }
    }

val replaceFallsThrough =
    fixturePatch("replace-falls-through") {
        anchorHost.method("replaced").replace { whenTrue(bool(true)) { returnVoid() } }
    }

private object GhostExtension : ExtClass("app.reseam.test.GhostExtension") {
    val run = static("run", Type.Boolean, returns = Type.Boolean)
}

val extMethodMissing =
    fixturePatch("ext-method-missing") {
        anchoredTwice.before { call(GhostExtension.run, bool(true)) }
    }

val assignFieldRead =
    fixturePatch("assign-field-read") {
        anchoredTwice.before { staticField(anchorValue).assign(int(7)) }
    }

val afterReturn =
    fixturePatch("after-return") {
        anchoredTwice
            .point { opcode(Opcode.RETURN_VOID) }
            .after { callStatic("com.example.Observer", "mark", "(I)V", int(1)) }
    }

val privateFieldAccess =
    fixturePatch("private-field-access") {
        anchorHost.classDef.addField(
            NewField(
                name = "secret",
                fieldType = Type.Int,
                accessFlags = (AccessFlags.PRIVATE or AccessFlags.STATIC).toUInt(),
                initialValue = null,
            )
        )
        klass("com.other.Peer").method("run").before {
            setStatic(FieldRef("Lcom/example/AnchorHost;", "secret", Type.Int), int(1))
        }
    }

val callArity =
    fixturePatch("call-arity") {
        anchoredTwice.before {
            callStatic("java.lang.String", "valueOf", "(I)Ljava/lang/String;")
        }
    }

val pointAnchors =
    fixturePatch("point-anchors") {
        anchoredPoint.after { mark(10) }
        anchoredPoint.after { mark(20) }
        anchoredPoint.before { mark(30) }
        anchoredTwice.before { mark(40) }
        anchoredPoint.after { mark(50) }

        val replaced = anchorHost.method("replaced")
        val lostPoint = replaced.point { literal(2) }
        replaced.alwaysReturn()
        replaced.reserveLocal("afterReplacement", Type.Int)
        val lost = runCatching { lostPoint.before { mark(60) } }
        check(lost.isFailure)
    }

val reservedLocals =
    fixturePatch("reserved-locals") {
        val carried = carry.reserveLocal("carried", Type.Int)
        val carriedWide = carry.reserveLocal("carriedWide", Type.Long)
        carry.before {
            callStatic("com.example.Observer", "scalar", "(I)V", local(carried))
            local(carried).assign(int(77))
            local(carriedWide).assign(long(0x1_0000_0001L))
        }
        carry
            .point { literal(8) }
            .after {
                callStatic("com.example.Observer", "pair", "(II)V", int(1), int(2))
            }
        carry
            .point { literal(9) }
            .after {
                callStatic("com.example.Observer", "scalar", "(I)V", local(carried))
                callStatic("com.example.Observer", "wide", "(J)V", local(carriedWide))
            }
    }

val capturedArguments =
    fixturePatch("captured-arguments") {
        greetCall
            .captureArgumentAs("host", 0)
            .captureArgumentAs("text", 1)
            .captureArgumentAs("count", 2)
            .before {
                callStatic(
                    "com.example.Observer",
                    "seen",
                    "(Lcom/example/ArgHost;Ljava/lang/String;I)V",
                    capture("host"),
                    capture("text"),
                    capture("count"),
                )
            }
        wideCall.captureArgumentAs("long", 1).captureArgumentAs("tail", 2).before {
            callStatic(
                "com.example.Observer",
                "wideSeen",
                "(JI)V",
                capture("long"),
                capture("tail"),
            )
        }
        val notInvoke = runCatching {
            argHost.method("run").point { literal(5) }.captureArgumentAs("x", 0).index
        }
        check(notInvoke.isFailure)
        val outOfRange = runCatching { greetCall.captureArgumentAs("x", 3).index }
        check(outOfRange.isFailure)
    }

val resourceEntries =
    fixturePatch("resource-entries") {
        val icon = resources.addFile("drawable", "reseam_icon", ICON_PATH, ICON_BYTES)
        check(resources.id("drawable", "reseam_icon") == icon) {
            "drawable/reseam_icon is not addressable"
        }
        resources.addFile("mipmap", "reseam_launcher", "res/reseam_launcher.png", ICON_BYTES)
        check(resources.path("mipmap", "reseam_launcher") == "res/reseam_launcher.png") {
            "mipmap path"
        }

        resources.addFile(
            "drawable",
            "reseam_pulse",
            "res/drawable/reseam_pulse.xml",
            PULSE_XML.encodeToByteArray(),
        )

        check(runCatching { resources.addFile("drawable", "absent", "res/absent.png") }.isFailure)
        check(runCatching { resources.path("string", "base_label") }.isFailure)

        resources.style("Theme.Test") {
            this["android:windowBackground"] = "@drawable/reseam_icon"
            this["android:textColor"] = "#ff102030"
        }
        resources.style("Theme.Reseam", parent = "@style/Theme.Test") {
            this["layout_constraintRight_toLeftOf"] = "@id/controls"
        }
        val orphan = runCatching {
            resources.style("Theme.Missing") { this["android:textColor"] = "#ffffffff" }
        }
        check(orphan.isFailure)

        val lengths = resources.getArray("double_tap_lengths")
        resources.setArray(
            "double_tap_lengths",
            lengths + listOf("15", "20", "@drawable/reseam_icon"),
        )
        resources.setStringArray(
            "double_tap_lengths",
            resources.getArray("double_tap_lengths") + listOf("true", "@string/missing"),
        )

        val typed =
            resources.getArrayValues("double_tap_lengths") +
                listOf(
                    ResourceValue(0x04u, 0x80000000u),
                    ResourceValue(0x05u, 0x00002101u),
                    ResourceValue(0x12u, 1u),
                    ResourceValue(0x01u, icon),
                )
        resources.setArrayValues("double_tap_lengths", typed)

        resources
            .component("config.test")
            .addFile(
                "xml",
                "split_settings",
                "res/split.xml",
                "<settings />".encodeToByteArray(),
            )
        files.write("res/split.xml", "<base />".encodeToByteArray())
        resources.editXml("xml", "split_settings") { root.appendChild(createElement("split")) }

        files.write(
            "assets/resource-entries.txt",
            "${resources.path("layout", "player_controls")}|${lengths.joinToString(",")}"
                .encodeToByteArray(),
        )
    }

val xmlGraft =
    declaredFixture("xml-graft") {
        enabledByDefault(false)
        dependsOn(resourceEntries)

        execute {
            XmlDocument.compile(CONTROL_FRAGMENT).use { fragment ->
                resources.editXml("layout", "player_controls") {
                    root.appendChild(adopt(fragment.root))
                    findByTag("ImageView").first()["app:layout_constraintRight_toLeftOf"] =
                        "@id/controls"
                }
                manifest.edit {
                    val foreign = runCatching { adopt(fragment.root) }
                    check(foreign.isFailure)
                    declareNamespace("app", RES_AUTO)
                    root["app:layout_constraintRight_toLeftOf"] = "@id/controls"
                }
            }
        }
    }

val registerWriters =
    fixturePatch("register-writers") {
        val single = takeCall("single").writer(0)
        single.after { mark(99) }
        val copied = takeCall("copied").writer(0)
        copied.after { mark(98) }
        val merged = runCatching { takeCall("merged").writer(0).index }
        val passed = runCatching { takeCall("passed").writer(0).index }
        val copiedParameter = runCatching { takeCall("copiedParameter").writer(0).index }
        files.write(
            "assets/register-writers.txt",
            listOf(merged.isFailure, passed.isFailure, copiedParameter.isFailure)
                .joinToString("\n")
                .encodeToByteArray(),
        )
    }

val wideCondition = invalidCondition("wide-condition") { whenTrue(long(1)) {} }
val primitiveNull = invalidCondition("primitive-null") { whenNull(int(1)) {} }
val mixedEquality = invalidCondition("mixed-equality") { whenEqual(int(1), string("one")) {} }
val primitiveInstance =
    invalidCondition("primitive-instance") { whenInstanceOf(int(1), Type.Object) {} }
val primitiveInstanceType =
    invalidCondition("primitive-instance-type") { whenInstanceOf(nullObject, Type.Int) {} }

val typedComparisons =
    fixturePatch("typed-comparisons") {
        val host = klass("com.example.ComparisonHost")
        for (type in listOf("long", "float", "double", "int", "reference")) {
            host.method("${type}Equal").replace {
                whenEqual(param(0), param(1)) { returnValue(int(1)) } otherwise
                    {
                        returnValue(int(0))
                    }
            }
            host.method("${type}NotEqual").replace {
                whenNotEqual(param(0), param(1)) { returnValue(int(1)) } otherwise
                    {
                        returnValue(int(0))
                    }
            }
        }
    }

val ownedCallbacks = run {
    lateinit var builder: app.reseam.patch.PatchBuilder
    patch("owned-callbacks") {
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

val primitiveConstruction = invalidCondition("primitive-construction") { newInstance(Type.Int) }
val invalidConstructorReturn =
    invalidCondition("invalid-constructor-return") { newInstance(Type.Object, "()I") }

val incompleteHierarchies =
    fixturePatch("incomplete-hierarchies") {
        val host = klass("com.example.TypeHost")
        for ((name, expected) in
            listOf(
                "application" to "Landroid/content/Context;",
                "image" to "Landroid/view/View;",
                "appView" to "Landroid/view/View;",
                "libraryActivity" to "Landroid/content/Context;",
                "missingSource" to "Lcom/example/KnownTarget;",
                "missingTarget" to "Lcom/example/ExternalTarget;",
                "knownSubclass" to "Lcom/example/KnownSource;",
            )) {
            host.method(name).before {
                callStatic("com.example.Observer", "accept", "($expected)V", param(0))
            }
        }
        val rejected =
            listOf(
                    "unrelated" to "Lcom/example/KnownTarget;",
                    "primitiveReference" to "Landroid/content/Context;",
                    "wideNarrow" to Type.Int,
                    "arrayElements" to "[J",
                )
                .map { (name, expected) ->
                    runCatching {
                        host.method(name).before {
                            callStatic(
                                "com.example.Observer",
                                "accept",
                                "($expected)V",
                                param(0),
                            )
                        }
                    }
                        .isFailure
                }
        files.write(
            "assets/type-validation.txt",
            rejected.joinToString("|").encodeToByteArray(),
        )
    }

val constantForms =
    fixturePatch("constant-forms") {
        val host = klass("com.example.ConstantHost")
        for ((name, value) in
            listOf(
                "small" to 7,
                "shortMax" to 32767,
                "shortMin" to -32768,
                "positiveWide" to 32768,
                "negativeWide" to -32769,
                "positiveHigh" to 65536,
                "negativeHigh" to -65536,
                "color" to -16777216,
                "minimum" to Int.MIN_VALUE,
                "maximum" to Int.MAX_VALUE,
            )) {
            host.method(name).replace {
                whenTrue(param(0)) { returnValue(int(value)) }
                returnValue(int(0))
            }
        }
    }
