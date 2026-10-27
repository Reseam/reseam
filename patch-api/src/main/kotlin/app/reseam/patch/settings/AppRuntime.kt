// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import app.reseam.patch.CodeScope
import app.reseam.patch.ExtClass
import app.reseam.patch.ExtJsModule
import app.reseam.patch.JsCodeScope
import app.reseam.patch.JsValue
import app.reseam.patch.Type
import app.reseam.patch.ValueRef
import app.reseam.patch.XmlElement
import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.method
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.types.HermesModule
import app.reseam.patch.types.RunningBundleInfo

/**
 * The app runtime's entry class. The engine provides the app runtime and links it into the app the
 * first time a patch uses it; bundles never ship it.
 */
object Reseam : ExtClass("app.reseam.runtime.Reseam") {
    /**
     * Starts an intent, or opens a bundle's settings when the intent comes from an element
     * [opensSettings] set up. Redirect the `startActivity` call of a settings row the app builds
     * from an intent, such as an androidx `Preference`, to this.
     */
    val startActivity by static(Type.Context, "Landroid/content/Intent;")

    internal val init by static(Type.Context)
    internal val isEnabled by static(Type.String, Type.String, Type.Boolean, returns = Type.Boolean)
    internal val getString by static(Type.String, Type.String, Type.String, returns = Type.String)
    internal val getChoice by static(Type.String, Type.String, Type.String, returns = Type.String)
    internal val openSettings by static(Type.String)
}

internal object ReseamReactPackage : ExtClass("app.reseam.runtime.react.ReseamReactPackage") {
    val addTo by static(Type.List)
}

/**
 * Adds the app runtime's React Native package, which gives the app's JavaScript the settings the
 * Hermes gates read, to [packages], the app's list of React Native packages. It adds it once
 * however many bundles call this, and makes it implement the app's package interface, which apps
 * built with R8 rename.
 */
fun CodeScope.addReactPackage(packages: ValueRef): ValueRef {
    val runtimePackage = ReseamReactPackage.target.classDef
    val reactPackage = createViewManagers.owner
    if (reactPackage !in runtimePackage.interfaces) runtimePackage.addInterface(reactPackage)
    return call(ReseamReactPackage.addTo, packages)
}

/**
 * `ReactPackage.createViewManagers`, which only the package interface declares abstract. The
 * interface's `createNativeModules` has a default implementation in some React Native versions.
 */
private val createViewManagers =
    method("ReactPackage.createViewManagers") {
        name("createViewManagers")
        params("com.facebook.react.bridge.ReactApplicationContext")
        returns(Type.List)
        flags(AccessFlags.ABSTRACT)
    }

internal object AppRuntimeJs : ExtJsModule("reseam-app-runtime-js") {
    override val source = HermesModule.AppRuntime
    val isEnabled by export()
    val withProperty by export()
}

/**
 * The running bundle's value of [setting] while the app runs. Code that runs before the app's
 * `Application.onCreate`, such as a content provider, reads the default.
 */
fun CodeScope.valueOf(setting: ToggleSetting): ValueRef =
    call(Reseam.isEnabled, string(bundle.name), string(setting.key), bool(setting.default))

/** The text setting's value; see the toggle's [valueOf]. */
fun CodeScope.valueOf(setting: TextSetting): ValueRef =
    call(Reseam.getString, string(bundle.name), string(setting.key), string(setting.default))

/** The folder setting's value; see the toggle's [valueOf]. */
fun CodeScope.valueOf(setting: FolderSetting): ValueRef =
    call(Reseam.getString, string(bundle.name), string(setting.key), string(setting.default))

/**
 * The choice setting's value; see the toggle's [valueOf]. A stored value the setting no longer
 * lists reads as the default.
 */
fun CodeScope.valueOf(setting: ChoiceSetting): ValueRef =
    call(Reseam.getChoice, string(bundle.name), string(setting.key), string(setting.default))

/** Opens the running bundle's settings over the app's current screen. */
fun CodeScope.openSettings(): ValueRef = call(Reseam.openSettings, string(bundle.name))

private const val OPEN_ACTION = "app.reseam.settings.OPEN"
private const val SCHEME = "reseam-settings"

/**
 * Makes this `<intent>` element of a settings row open the running bundle's settings, once the
 * row's `startActivity` call is redirected to [Reseam.startActivity].
 */
fun XmlElement.opensSettings() {
    this["android:action"] = OPEN_ACTION
    this["android:data"] = "$SCHEME:${bundle.name}"
}

internal val bundle: RunningBundleInfo
    get() = ActiveRuntime.current.bundle

/** The running bundle's value of [setting] while the app runs. */
fun JsCodeScope.valueOf(setting: ToggleSetting): JsValue =
    call(AppRuntimeJs.isEnabled, string(bundle.name), string(setting.key), bool(setting.default))
