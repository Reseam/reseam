// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.CodeScope
import app.reseam.patch.PatchRuntime
import app.reseam.patch.Type
import app.reseam.patch.before
import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.opcode
import app.reseam.patch.klass
import app.reseam.patch.method
import app.reseam.patch.methods
import app.reseam.patch.native.FieldRef
import app.reseam.patch.patch
import app.reseam.patch.point

internal val appVideoState = klass("appVideoState") { strings("NEW", "PLAYING") }

internal val anyVideoState =
    klass("anyVideoState") {
        strings("NEW", "PLAYING")
        includeExtensions()
        first()
    }

internal val flagHolder = klass("com.example.FlagHolder")

internal val publicFinalHook =
    method("publicFinalHook") {
        inClass(flagHolder)
        params()
        returns(Type.Void)
        flags(AccessFlags.PUBLIC or AccessFlags.FINAL)
    }

internal val shapeHolder = klass("com.example.ShapeHolder")

internal val unorderedShape =
    shapeHolder.methods("unorderedShape") { opcode(Opcode.CONST, Opcode.CONST_STRING) }

internal val orderedShape =
    method("orderedShape") {
        inClass(shapeHolder)
        opcodeSequence(Opcode.CONST, null, Opcode.RETURN_VOID)
    }

internal val shortestMethod =
    method("shortestMethod") {
        inClass(shapeHolder)
        custom { instructionCount == 1 }
    }

internal val binderClass = klass("binderClass") { sourceFile("LithoRVSLCBinder.java") }

internal val preferenceFragment = klass("com.example.PreferenceFragment")

internal val inheritedFindPreference = preferenceFragment.method("findPreference", inherited = true)

internal val findPreferenceCallee =
    klass("com.example.Caller")
        .method("run")
        .point { invokeVirtual { name("findPreference") } }
        .callee()

internal fun CodeScope.mark(value: Int) {
    callStatic("com.example.Observer", "mark", "(I)V", int(value))
}

internal val skipHost = klass("com.example.SkipHost")

internal val anchorHost = klass("com.example.AnchorHost")

internal val anchoredTwice = anchorHost.method("twice")

internal val anchoredPoint = anchoredTwice.point { literal(2) }

internal val anchorValue = FieldRef("Lcom/example/AnchorHost;", "value", Type.Int)

internal val carry = klass("com.example.LocalHost").method("carry")

internal val argHost = klass("com.example.ArgHost")

internal val greetCall = argHost.method("run").point { invokeVirtual { name("greet") } }

internal val wideCall = argHost.method("wide").point { invokeStatic { name("wide2") } }

internal const val ICON_PATH = "res/reseam_icon.png"

internal val ICON_BYTES = byteArrayOf(0x89.toByte(), 0x50, 0x4e, 0x47)

internal const val PULSE_XML =
    """<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt">
    <aapt:attr name="android:drawable"><vector android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24"><group android:name="icon"/></vector></aapt:attr>
    <target android:name="icon"><aapt:attr name="android:animation"><objectAnimator android:propertyName="scaleX" android:valueTo="1.1"/></aapt:attr></target>
</animated-vector>"""

internal const val RES_AUTO = "http://schemas.android.com/apk/res-auto"

internal val CONTROL_FRAGMENT =
    """
    <FrameLayout xmlns:android="http://schemas.android.com/apk/res/android"
        xmlns:yt="$RES_AUTO"
        xmlns:tools="http://schemas.android.com/tools"
        android:id="@+id/reseam_button_container"
        android:layout_width="48dp"
        tools:ignore="ContentDescription"
        yt:layout_constraintRight_toLeftOf="@id/fullscreen_button">
        <ImageView android:id="@+id/reseam_button" android:src="@drawable/reseam_icon" />
    </FrameLayout>
"""
        .trimIndent()

internal val writerHost = klass("com.example.WriterHost")

internal fun takeCall(method: String) =
    writerHost.method(method).point { invokeStatic { name("take") } }

internal fun invalidCondition(name: String, block: CodeScope.() -> Unit) =
    patch(name) {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        execute { anchoredTwice.before(block) }
    }

internal fun fixturePatch(name: String, execute: PatchRuntime.() -> Unit) =
    patch(name) {
        description(fixtureDescriptions.getValue(name))
        compatibleWith("com.example.test")
        enabledByDefault(false)
        execute(execute)
    }

internal val fixtureDescriptions: Map<String, String> =
    java.util.Properties().run {
        checkNotNull(
                RuntimeFixtures::class.java.getResourceAsStream("/fixtures/runtime.properties")
            )
            .use(::load)
        stringPropertyNames().associateWith { getProperty(it) }
    }

private object RuntimeFixtures

internal fun declaredFixture(name: String, block: app.reseam.patch.PatchBuilder.() -> Unit) =
    patch(name) {
        description(fixtureDescriptions[name] ?: "")
        compatibleWith("com.example.test")
        block()
    }
