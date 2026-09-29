// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.CodeScope
import app.reseam.patch.ExtClass
import app.reseam.patch.ExternalPatch
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
import app.reseam.patch.native.FieldRef
import app.reseam.patch.native.NewField
import app.reseam.patch.methods
import app.reseam.patch.patch
import app.reseam.patch.point
import app.reseam.patch.replace
import app.reseam.patch.reserveLocal
import app.reseam.patch.skipWhen
import app.reseam.patch.settings.section
import app.reseam.patch.settings.settingsHost
import app.reseam.patch.settings.ToggleSetting

val finalizeOwner = patch("finalize-owner") {
    description("Exercises afterDependents through the real Kotlin runtime")
    compatibleWith("com.example.test")

    execute {
        log.info("finalize-owner execute")
    }

    afterDependents {
        manifest.addPermission("android.permission.INTERNET")
    }
}

val runtimeApi = patch("runtime-api") {
    description("Exercises PatchRuntime scopes against split APK state")
    compatibleWith("com.example.test")
    val baseVersion = stringOption("baseVersion", default = "2.0-base")
    val splitVersion = stringOption("splitVersion", default = "2.0-split")
    val splitText = stringOption("splitText", default = "Split patched")

    execute {
        manifest.setVersionName(options[baseVersion])
        manifest.component("config.test").setVersionName(options[splitVersion])
        resources.setString("split_label", options[splitText])
        files.write("assets/base-marker.txt", "base".encodeToByteArray())
        files.component("config.test").write("assets/split-marker.txt", "split".encodeToByteArray())
    }
}

val dependentRuntime = patch("dependent-runtime") {
    description("Depends on finalize-owner to trigger afterDependents")
    compatibleWith("com.example.test")
    dependsOn(finalizeOwner)

    execute {
        files.component("config.test").write("assets/dependent-marker.txt", "dependent".encodeToByteArray())
    }
}

val requiredOption = patch("required-option") {
    description("Used to verify option validation against real Kotlin patches")
    compatibleWith("com.example.test")
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

val usesInternal = patch("uses-internal") {
    description("Pulls the internal helper in as a dependency")
    compatibleWith("com.example.test")
    dependsOn(internalHelper)

    execute {
        log.info("uses-internal execute")
    }
}

val afterEntryValues = patch("after-entry-values") {
    description("Checks entry argument lifetimes through the real code emitter")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        val target = klass("com.example.HookTarget")
        check(target.method("invokeGrowth").method.growLocalRegisters(10))
        target.method("invokeGrowth").after {
            callStatic("com.example.Observer", "entry", "(I)V", param(3))
        }
        val receiver = target.method("receiver").method
        val noRegister = runCatching { receiver.findFreeRegister(0, (0 until receiver.registersSize).toList()) }
        check(noRegister.exceptionOrNull()?.message?.contains("No free register") == true)
        target.method("temporaryReuse").after {
            val held = long(0x123456789abcdef0L)
            repeat(32) {
                callStatic("com.example.Observer", "wide", "(J)V", long(it.toLong()))
                callStatic("com.example.Observer", "scalar", "(I)V", int(it))
            }
            whenTrue(param(0)) {
                callStatic("com.example.Observer", "wide", "(J)V", held)
            } otherwise {
                callStatic("com.example.Observer", "wide", "(J)V", held)
            }
            capture("result").assign(held)
        }
        target.method("getFeatureSwitchValue").after {
            val marker = int(42)
            callStatic("com.example.Observer", "record", "(ILjava/lang/String;JDLjava/lang/String;Ljava/lang/String;Ljava/lang/Object;)V",
                marker, param(0), paramOfType(Type.Long), param(2), lastParam, paramOfType(Type.String), capture("result"))
        }
        target.method("receiver").after {
            callStatic("com.example.Observer", "receiver", "(Lcom/example/HookTarget;)V", thisObject)
        }
        target.method("resultOnly").after {
            capture("result").assign(int(42))
        }
    }
}

val embeddedStrings = patch("embedded-strings") {
    description("Rewrites constants that only contain the value, leaving the ones transform declines")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        val changed = bytecode.replaceStringsContaining("com.google.android.gsf") { old ->
            old.takeIf { it.startsWith("content://") }?.replace("com.google.android.gsf", "app.reseam.gsf")
        }
        files.write("assets/embedded-strings.txt", changed.toString().encodeToByteArray())
    }
}

val duplicateSettings = settingsHost("duplicates") {
    compatibleWith("com.example.test")
    install { }
}

val firstAds = patch("Hide Ads") {
    compatibleWith("com.example.test")
    enabledByDefault(false)
    val marker = stringOption("marker", default = "first")
    val label = stringOption("label.detail", default = "nested")
    settings(duplicateSettings, section("First", ToggleSetting("first.enabled", "Enabled", default = true)))
    execute {
        files.write("assets/first-ads.txt", options[marker].encodeToByteArray())
        files.write("assets/first-label.txt", options[label].encodeToByteArray())
    }
}

val firstAdsAlias = firstAds

val secondAds = patch("Hide Ads") {
    compatibleWith("com.example.test")
    enabledByDefault(false)
    dependsOn(firstAds)
    val marker = stringOption("marker", default = "second")
    settings(duplicateSettings, section("Second", ToggleSetting("second.enabled", "Enabled", default = true)))
    execute { files.write("assets/second-ads.txt", options[marker].encodeToByteArray()) }
}

val otherAds = patch("Hide Ads") {
    compatibleWith("com.example.other")
    enabledByDefault(false)
    val marker = stringOption("marker", default = "other")
    execute { files.write("assets/other-ads.txt", options[marker].encodeToByteArray()) }
}

val otherBundleHelper = ExternalPatch("other-bundle", "app.reseam.other.helper")

val needsOtherBundle = patch("needs-other-bundle") {
    description("Depends on a patch from a bundle that is not loaded")
    compatibleWith("com.example.test")
    dependsOn(otherBundleHelper)
    execute { }
}

val universalMarker = patch("universal-marker") {
    description("Declares no package, so it works with any app and waits to be asked for")
    execute { files.write("assets/universal-marker.txt", "universal".encodeToByteArray()) }
}

val appEntryHook = patch("app-entry-hook") {
    description("Hands the Application to com.example.Observer at app start")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute { appEntry { callStatic("com.example.Observer", "started", "(Landroid/app/Application;)V", application) } }
}

val unwrapApplication = patch("unwrap-application") {
    description("Names the wrapper Application's superclass in the manifest after appEntry code was added")
    compatibleWith("com.example.test")
    enabledByDefault(false)
    dependsOn(appEntryHook)

    execute {
        manifest.edit {
            val application = findByTag("application").single()
            application["android:name"] = bytecode.findClass(application["android:name"]!!)!!.superclass!!
                .removePrefix("L").removeSuffix(";").replace('/', '.')
        }
    }
}

private val appVideoState = klass("appVideoState") { strings("NEW", "PLAYING") }

private val anyVideoState = klass("anyVideoState") {
    strings("NEW", "PLAYING")
    includeExtensions()
    first()
}

val extensionShadow = patch("extension-shadow") {
    description("A query sees app classes, not the extension classes the bundle links in")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        // Referencing the class links its extension DEX, which is what puts its strings in the app.
        check(bytecode.findClass("app.reseam.test.ext.VideoState") != null)
        // A named owner keeps an extension class's own methods reachable through a query.
        check(klass("app.reseam.test.ext.VideoState").method("names").owner == "Lapp/reseam/test/ext/VideoState;")
        files.write(
            "assets/extension-shadow.txt",
            "${appVideoState.descriptor}|${anyVideoState.descriptor}".encodeToByteArray(),
        )
    }
}

private val flagHolder = klass("com.example.FlagHolder")

private val publicFinalHook = method("publicFinalHook") {
    inClass(flagHolder)
    params()
    returns(Type.Void)
    flags(AccessFlags.PUBLIC or AccessFlags.FINAL)
}

val allAccessFlags = patch("all-access-flags") {
    description("flags() needs every bit, so a private final method beside a public final one is no match")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute { files.write("assets/all-access-flags.txt", publicFinalHook.name.encodeToByteArray()) }
}

val xmlAttributeBinding = patch("xml-attribute-binding") {
    description("An attribute name the document never carried still binds the id the inflater reads")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        manifest.edit {
            root.appendChild(
                createElement("intent").apply {
                    this["android:targetPackage"] = "com.example.target"
                    this["android:targetClass"] = "com.example.target.Settings"
                },
            )
            val unknown = runCatching { root["android:notAnAttribute"] = "x" }
            check(unknown.exceptionOrNull()?.message?.contains("no id for it") == true) { "unknown android attribute: $unknown" }
            val undeclared = runCatching { root["yt:layout_constraintLeft_toLeftOf"] = "x" }
            check(undeclared.exceptionOrNull()?.message?.contains("declares no xmlns:yt; declare it with declareNamespace") == true) { "undeclared prefix: $undeclared" }
        }
    }
}

private val shapeHolder = klass("com.example.ShapeHolder")

private val unorderedShape = shapeHolder.methods("unorderedShape") { opcode(Opcode.CONST, Opcode.CONST_STRING) }

private val orderedShape = method("orderedShape") {
    inClass(shapeHolder)
    opcodeSequence(Opcode.CONST, null, Opcode.RETURN_VOID)
}

private val shortestMethod = method("shortestMethod") {
    inClass(shapeHolder)
    custom { instructionCount == 1 }
}

val instructionShapeQueries = patch("instruction-shape-queries") {
    description("Instruction order and a custom predicate separate methods nothing else tells apart")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        // The same opcodes in either order, which is all an unordered opcode() sees.
        check(unorderedShape.all.size == 2) { "expected both shapes, got ${unorderedShape.all.map { it.descriptor }}" }
        files.write(
            "assets/instruction-shape.txt",
            "${orderedShape.name}|${shortestMethod.name}".encodeToByteArray(),
        )
    }
}

private val binderClass = klass("binderClass") { sourceFile("LithoRVSLCBinder.java") }

val sourceFileQuery = patch("source-file-query") {
    description("A class the obfuscator left a source file name on is reachable by that name")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute { files.write("assets/source-file.txt", binderClass.descriptor.encodeToByteArray()) }
}

private val preferenceFragment = klass("com.example.PreferenceFragment")

private val inheritedFindPreference = preferenceFragment.method("findPreference", inherited = true)

private val findPreferenceCallee = klass("com.example.Caller")
    .method("run")
    .point { invokeVirtual { name("findPreference") } }
    .callee()

val inheritedMethods = patch("inherited-methods") {
    description("A method an obfuscated base declares is reachable from the subclass it is called on")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        val declaredOnly = runCatching { preferenceFragment.method("findPreference").method }
        check(declaredOnly.exceptionOrNull()?.message?.contains("inherited method(s)") == true) {
            "declared-only lookup should name the inherited methods it skipped: $declaredOnly"
        }
        files.write(
            "assets/inherited-methods.txt",
            "${inheritedFindPreference.owner}|${findPreferenceCallee.owner}".encodeToByteArray(),
        )
    }
}

private fun CodeScope.mark(value: Int) {
    callStatic("com.example.Observer", "mark", "(I)V", int(value))
}

private val skipHost = klass("com.example.SkipHost")

val skipWhenCall = patch("skip-when") {
    description("Guards one call behind a condition, on every path into it")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        skipHost.method("straight").point { literal(2) }.next { invokeStatic { name("mark") } }.skipWhen { bool(true) }
        skipHost.method("joined").point { literal(2) }.next { invokeStatic { name("mark") } }.next { invokeStatic { name("mark") } }
            .skipWhen { bool(true) }
        skipHost.method("joinedKept").point { literal(2) }.next { invokeStatic { name("mark") } }.next { invokeStatic { name("mark") } }
            .skipWhen { bool(false) }
    }
}

val beforeJoin = patch("before-join") {
    description("Runs code before a call that a branch jumps to")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        skipHost.method("joinedBefore").point { literal(2) }.next { invokeStatic { name("mark") } }
            .next { invokeStatic { name("mark") } }.before { mark(9) }
    }
}

val whenInstanceOfBranch = patch("when-instance-of") {
    description("Branches on the runtime type of a parameter")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        skipHost.method("typed").replace {
            whenInstanceOf(param(0), "java.lang.String") { mark(1) } otherwise { mark(2) }
            returnVoid()
        }
    }
}

private val anchorHost = klass("com.example.AnchorHost")

val replaceBothReturn = patch("replace-both-return") {
    description("A replaced body whose then and else blocks both return needs no jump over the else block")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        anchorHost.method("replaced").replace {
            whenTrue(bool(true)) { returnVoid() } otherwise { returnVoid() }
        }
    }
}

val replaceFallsThrough = patch("replace-falls-through") {
    description("A replaced body that falls off its end fails the patch")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute { anchorHost.method("replaced").replace { whenTrue(bool(true)) { returnVoid() } } }
}
private val anchoredTwice = anchorHost.method("twice")
private val anchoredPoint = anchoredTwice.point { literal(2) }

private object GhostExtension : ExtClass("app.reseam.test.GhostExtension") {
    val run = static("run", Type.Boolean, returns = Type.Boolean)
}

val extMethodMissing = patch("ext-method-missing") {
    description("Calls an extension method no extension defines")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute { anchoredTwice.before { call(GhostExtension.run, bool(true)) } }
}

private val anchorValue = FieldRef("Lcom/example/AnchorHost;", "value", Type.Int)

val setStaticField = patch("set-static") {
    description("Writes a static field")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute { anchoredTwice.before { setStatic(anchorValue, int(7)) } }
}

val assignFieldRead = patch("assign-field-read") {
    description("Assigns to a value read from a field, which only overwrites a copy")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute { anchoredTwice.before { staticField(anchorValue).assign(int(7)) } }
}

val afterReturn = patch("after-return") {
    description("Hooks the code after a return, which never runs")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        anchoredTwice.point { opcode(Opcode.RETURN_VOID) }
            .after { callStatic("com.example.Observer", "mark", "(I)V", int(1)) }
    }
}

val privateFieldAccess = patch("private-field-access") {
    description("Writes another class's private field, which the runtime refuses")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        anchorHost.classDef.addField(
            NewField(
                name = "secret",
                fieldType = Type.Int,
                accessFlags = (AccessFlags.PRIVATE or AccessFlags.STATIC).toUInt(),
                initialValue = null,
            ),
        )
        klass("com.other.Peer").method("run").before {
            setStatic(FieldRef("Lcom/example/AnchorHost;", "secret", Type.Int), int(1))
        }
    }
}

val callArity = patch("call-arity") {
    description("Passes the wrong number of arguments to a call")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute { anchoredTwice.before { callStatic("java.lang.String", "valueOf", "(I)Ljava/lang/String;") } }
}

val pointAnchors = patch("point-anchors") {
    description("A point keeps naming its instruction through every edit its method takes after it resolved")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        anchoredPoint.after { mark(10) }
        anchoredPoint.after { mark(20) }
        anchoredPoint.before { mark(30) }
        anchoredTwice.before { mark(40) }
        anchoredPoint.after { mark(50) }

        val replaced = anchorHost.method("replaced")
        val lostPoint = replaced.point { literal(2) }
        check(lostPoint.index == 1)
        replaced.alwaysReturn()
        replaced.reserveLocal("afterReplacement", Type.Int)
        val lost = runCatching { lostPoint.before { mark(60) } }
        check(lost.exceptionOrNull()?.message?.contains("the method body was replaced under it") == true) { "stale point: $lost" }
    }
}

private val carry = klass("com.example.LocalHost").method("carry")

val reservedLocals = patch("reserved-locals") {
    description("A reserved register carries a value between blocks, past one that needs two temporaries at once")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        val carried = carry.reserveLocal("carried", Type.Int)
        val carriedWide = carry.reserveLocal("carriedWide", Type.Long)
        carry.before {
            callStatic("com.example.Observer", "scalar", "(I)V", local(carried))
            local(carried).assign(int(77))
            local(carriedWide).assign(long(0x1_0000_0001L))
        }
        carry.point { literal(8) }.after {
            callStatic("com.example.Observer", "pair", "(II)V", int(1), int(2))
        }
        carry.point { literal(9) }.after {
            callStatic("com.example.Observer", "scalar", "(I)V", local(carried))
            callStatic("com.example.Observer", "wide", "(J)V", local(carriedWide))
        }
    }
}

private val argHost = klass("com.example.ArgHost")
private val greetCall = argHost.method("run").point { invokeVirtual { name("greet") } }
private val wideCall = argHost.method("wide").point { invokeStatic { name("wide2") } }

val capturedArguments = patch("captured-arguments") {
    description("captureArgumentAs picks the register an invoke passes: the receiver first, a wide argument once")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        greetCall.captureArgumentAs("host", 0).captureArgumentAs("text", 1).captureArgumentAs("count", 2).before {
            callStatic(
                "com.example.Observer", "seen", "(Lcom/example/ArgHost;Ljava/lang/String;I)V",
                capture("host"), capture("text"), capture("count"),
            )
        }
        wideCall.captureArgumentAs("long", 1).captureArgumentAs("tail", 2).before {
            callStatic("com.example.Observer", "wideSeen", "(JI)V", capture("long"), capture("tail"))
        }
        val notInvoke = runCatching { argHost.method("run").point { literal(5) }.captureArgumentAs("x", 0).index }
        check(notInvoke.exceptionOrNull()?.message?.contains("is not an invoke, so it passes no arguments") == true) { "non-invoke: $notInvoke" }
        val outOfRange = runCatching { greetCall.captureArgumentAs("x", 3).index }
        check(outOfRange.exceptionOrNull()?.message?.contains("so there is no argument 3") == true) { "out of range: $outOfRange" }
    }
}

private const val ICON_PATH = "res/reseam_icon.png"
private val ICON_BYTES = byteArrayOf(0x89.toByte(), 0x50, 0x4e, 0x47)
private const val PULSE_XML = """<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt">
    <aapt:attr name="android:drawable"><vector android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24"><group android:name="icon"/></vector></aapt:attr>
    <target android:name="icon"><aapt:attr name="android:animation"><objectAnimator android:propertyName="scaleX" android:valueTo="1.1"/></aapt:attr></target>
</animated-vector>"""

val resourceEntries = patch("resource-entries") {
    description("Reaches a resource file by name, registers typed file resources, and edits a style and an array")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        val icon = resources.addFile("drawable", "reseam_icon", ICON_PATH, ICON_BYTES)
        check(resources.id("drawable", "reseam_icon") == icon) { "drawable/reseam_icon is not addressable" }
        // mipmap ships only a density configuration, so the entry has to make the default one.
        resources.addFile("mipmap", "reseam_launcher", "res/reseam_launcher.png", ICON_BYTES)
        check(resources.path("mipmap", "reseam_launcher") == "res/reseam_launcher.png") { "mipmap path" }

        resources.addFile("drawable", "reseam_pulse", "res/drawable/reseam_pulse.xml", PULSE_XML.encodeToByteArray())

        val unwritten = runCatching { resources.addFile("drawable", "absent", "res/absent.png") }
        check(unwritten.exceptionOrNull()?.message?.contains("has no entry res/absent.png") == true) { "unwritten file: $unwritten" }
        val notAFile = runCatching { resources.path("string", "base_label") }
        check(notAFile.exceptionOrNull()?.message?.contains("is not file-backed") == true) { "string entry: $notAFile" }

        resources.style("Theme.Test") {
            this["android:windowBackground"] = "@drawable/reseam_icon"
            this["android:textColor"] = "#ff102030"
        }
        resources.style("Theme.Reseam", parent = "@style/Theme.Test") {
            this["layout_constraintRight_toLeftOf"] = "@id/controls"
        }
        val orphan = runCatching { resources.style("Theme.Missing") { this["android:textColor"] = "#ffffffff" } }
        check(orphan.exceptionOrNull()?.message?.contains("pass a parent to create it") == true) { "no parent: $orphan" }

        val lengths = resources.getArray("double_tap_lengths")
        resources.setArray("double_tap_lengths", lengths + listOf("15", "20", "@drawable/reseam_icon"))
        resources.setStringArray("double_tap_lengths", resources.getArray("double_tap_lengths") + listOf("true", "@string/missing"))

        resources.component("config.test").addFile("xml", "split_settings", "res/split.xml", "<settings />".encodeToByteArray())
        files.write("res/split.xml", "<base />".encodeToByteArray())
        resources.editXml("xml", "split_settings") { root.appendChild(createElement("split")) }

        files.write(
            "assets/resource-entries.txt",
            "${resources.path("layout", "player_controls")}|${lengths.joinToString(",")}".encodeToByteArray(),
        )
    }
}

private const val RES_AUTO = "http://schemas.android.com/apk/res-auto"

private val CONTROL_FRAGMENT = """
    <FrameLayout xmlns:android="http://schemas.android.com/apk/res/android"
        xmlns:yt="$RES_AUTO"
        xmlns:tools="http://schemas.android.com/tools"
        android:id="@+id/reseam_button_container"
        android:layout_width="48dp"
        tools:ignore="ContentDescription"
        yt:layout_constraintRight_toLeftOf="@id/fullscreen_button">
        <ImageView android:id="@+id/reseam_button" android:src="@drawable/reseam_icon" />
    </FrameLayout>
""".trimIndent()

val xmlGraft = patch("xml-graft") {
    description("Grafts a bundled layout fragment into an app layout and binds an app attribute on it")
    compatibleWith("com.example.test")
    enabledByDefault(false)
    dependsOn(resourceEntries)

    execute {
        XmlDocument.compile(CONTROL_FRAGMENT).use { fragment ->
            resources.editXml("layout", "player_controls") {
                root.appendChild(adopt(fragment.root))
                // The app's own attribute on a document that already declares res-auto.
                findByTag("ImageView").first()["app:layout_constraintRight_toLeftOf"] = "@id/controls"
            }
            manifest.edit {
                val foreign = runCatching { adopt(fragment.root) }
                check(foreign.exceptionOrNull()?.message?.contains("declares no namespace $RES_AUTO") == true) {
                    "adopt into a document without res-auto: $foreign"
                }
                declareNamespace("app", RES_AUTO)
                root["app:layout_constraintRight_toLeftOf"] = "@id/controls"
            }
        }
    }
}

private val writerHost = klass("com.example.WriterHost")

private fun takeCall(method: String) = writerHost.method(method).point { invokeStatic { name("take") } }

val registerWriters = patch("register-writers") {
    description("writer(argument) walks back through register moves to the one instruction that wrote an invoke's argument, or says why there is none")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        val single = takeCall("single").writer(0)
        single.after { mark(99) }
        val copied = takeCall("copied").writer(0)
        copied.after { mark(98) }
        val merged = runCatching { takeCall("merged").writer(0).index }
        val passed = runCatching { takeCall("passed").writer(0).index }
        val copiedParameter = runCatching { takeCall("copiedParameter").writer(0).index }
        files.write(
            "assets/register-writers.txt",
            listOf(
                "${single.index}:${single.instruction.opcode}",
                merged.exceptionOrNull()?.message,
                passed.exceptionOrNull()?.message,
                "${copied.index}:${copied.instruction.opcode}",
                copiedParameter.exceptionOrNull()?.message,
            )
                .joinToString("\n")
                .encodeToByteArray(),
        )
    }
}
