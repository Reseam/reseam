---
description: Add a settings screen to the patched app, so users can turn features on and off.
---

# Settings inside the app

[Options](4_patches.md#options) are chosen once, before patching. Settings live inside the patched app: your bundle gets a settings screen in the app, and patched code checks the settings while the app runs. Users can change them at any time, without patching again.

Settings take three parts:

1. **Settings**, declared in Kotlin in a settings module, with their text in string resources.
2. **A settings host**, an internal patch that adds your bundle's settings screen to the app.
3. **Gates**, patched code that reads a setting while the app runs.

## The app runtime

The settings screen, the storage patched code reads, and the [branded components](#branded-components) come from the *app runtime*, which the engine provides. The engine adds it to the app the first time a patch uses it. Your bundle ships none of it, and any number of bundles share it in one app. It adds nothing to the manifest, so it works in installed and [mount builds](8_app_files.md#mount-builds) alike.

The app needs an `Application` class, because the settings host starts the app runtime first in its `onCreate`.

## Declare settings

Settings live in a *settings module*, `apps/<app>/settings/` (or `shared/<name>/settings/` for settings several apps share). The app's patches and extensions both use it.

```text
apps/example/settings/
  build.gradle.kts
  src/main/kotlin/app/example/settings/AppSettings.kt
  src/main/strings/values/strings.xml
```

`build.gradle.kts` names the object the build generates from your strings:

```kotlin
reseamStrings { objectName = "app.example.settings.ExampleStrings" }
```

Declare settings as properties of an object:

```kotlin
object AppSettings : SettingsGroup(ExampleStrings) {
    val hideAds by toggle(default = true)
    val feedStyle by choice(
        default = "compact",
        choices = listOf(
            Choice("compact", ExampleStrings.feedStyleCompact),
            Choice("cards", ExampleStrings.feedStyleCards),
        ),
    )
}
```

And their text in `strings.xml`:

```xml
<resources xmlns:xliff="urn:oasis:names:tc:xliff:document:1.2">
    <string name="hide_ads">Hide ads</string>
    <string name="hide_ads_summary">Removes ads from the feed.</string>
    <string name="feed_style">Feed style</string>
    <string name="feed_style_compact">Compact</string>
    <string name="feed_style_cards">Cards</string>
</resources>
```

A setting's title is the string named after its property in snake case: `hideAds` reads `hide_ads`. Its summary is the same name with `_summary`, when there is one. A toggle can also have `_summary_on` and `_summary_off`, and its row shows the one for its value, or `_summary` when that one is missing. A missing title fails the build. The generated `ExampleStrings` has one member per string, for choices, [sections](#show-settings-on-the-screen), and [pages](#pages).

There are eight kinds of setting that hold a value, each with a default and an optional [icon](#icons). Actions and notes hold none:

| Kind | Holds |
|---|---|
| `toggle(default)` | on or off |
| `text(default, multiline = false)` | text the user types. A `multiline` text holds one entry per line. |
| `number(default, slider)` | a number the user picks on a slider |
| `range(default, slider)` | a start and an end the user picks on one slider with two thumbs |
| `color(default)` | a color the user picks, as ARGB |
| `folder(default)` | a folder the user picks, with permission to read and write it |
| `file(default, mimeTypes)` | a file of one of `mimeTypes` the user picks, with permission to read it |
| `choice(default, choices)` | one of several values |
| `action()` | nothing. The row runs code when tapped, as [Actions](#actions) shows. |
| `note()` | nothing. The row shows its summary. |

Numbers, ranges and colors take what they are picked with:

```kotlin
val overlayOpacity by number(default = 60.0, slider = Slider(0.0, 100.0, 1.0, "%"))
val seekbarPositions by range(default = 0.7..1.0, slider = Slider(0.0, 1.0, 0.01))
val accentColor by color(default = 0xFFFF0033)
```

Extensions read them as a `Double`, a `NumberRange` and an ARGB `Int`. A stored value off the slider reads as the default, so code never sees a number the slider can't pick.

### Keys

A setting is stored under a key made from the object and property names: `AppSettings.hideAds` is stored as `app_settings.hide_ads`. Each bundle stores its settings apart, so keys only need to be unique within your bundle.

> [!WARNING]
> **Pin your keys before the first release.** If you later rename the object or the property, the key changes, and every user's saved value is lost. Set `key = "..."` on each setting now, and you can rename the code freely.

### One switch for several

A toggle can cover other toggles, as one switch for a whole feature:

```kotlin
val hideStories by toggle(default = false)
val hideReels by toggle(default = false)
val focusMode by toggle(default = false, covers = listOf(hideStories, hideReels))
```

While `focusMode` is on, `hideStories` and `hideReels` read as on everywhere, gates included. They show on and greyed out on the settings screen. Their own values are kept, and apply again when `focusMode` is turned off.

Declare the covered toggles first. Kotlin initializes properties in order, so a toggle used before its declaration is still null.

### When a change applies

Most patches read a setting once, when the app starts, so a change only shows after a restart. When the user closes the settings after changing such a setting, the screen offers to restart the app. The settings screen also has a restart button in its top bar.

A setting your code reads each time it is used, such as a folder read when a download starts, applies right away. Declare it with `takesEffect = TakesEffect.IMMEDIATELY`, and changing it never asks for a restart:

```kotlin
val downloadFolder by folder(default = "", takesEffect = TakesEffect.IMMEDIATELY)
```

Settings default to `TakesEffect.ON_RESTART`. Only declare `IMMEDIATELY` when every place that reads the setting reads it again after a change, or users see the old value until they restart anyway.

## Translations

Write the default strings in English, and put translations next to them, in Android's folder names: `values-de/strings.xml`, `values-pt-rBR/strings.xml`, `values-b+sr+Latn/strings.xml`. The app shows each string in the first of the user's languages that translates it, with Android's per-app language first when one is set. This works whether or not the app itself is translated into that language. A string no translation covers shows its default text. Users can also pick a language for Reseam's text alone from the settings screen, which then comes first. A translation counts for a language when it has the same script: `values-zh-rTW` serves Traditional Chinese in Taiwan and Hong Kong, and `values-pt` any Portuguese. English readers get the default text.

Reseam's settings screens lay out right to left in Arabic, Persian, Urdu and other right-to-left languages, also in apps that never declared right-to-left support.

- Escape apostrophes and quotes as `\'` and `\"`.
- Wrap each argument of a string in `xliff:g`, whose `id` names it: `Saved to <xliff:g id="folder">%1$s</xliff:g>`. `%s` takes text, `%d` a whole number, and `%f` a decimal number.
- `<plurals>` picks its form by an integer argument with the id `count`.

- Mark a string that reads the same in every language, such as a host name, with `translatable="false"`. Translation tools then skip it.

A translation must keep the default's arguments. One that adds, drops, or changes the type of an argument fails the build, and so does a translation of a string the default lacks or marks `translatable="false"`.

Titles, summaries, sections and pages show their text as it is, so they take strings without arguments. A string with an argument, or a `<plurals>`, is only in the extension-side object, and a setting named after one fails the build with a missing title.

### Styled strings

A string can use the styling tags of Android styled strings: `<b>`, `<i>`, `<u>`, `<small>`, `<big>`, `<sup>`, `<sub>`, and `<font>` with a `color`. The settings screen shows them wherever the string appears, in a title, a summary or a choice:

```xml
<string name="hide_shelves_summary">Hides shelves such as <b>Breaking news</b>.\nOther feeds stay.</string>
```

A line break is `\n`, as in any string. Other tags, and tags with attributes, fail the build.

Things to know:

- A translation of a styled string may be plain, and shows as it is. A translation of a plain string can't add styling, since the generated member's type follows the default, so that fails the build.
- A `<plurals>` can't be styled, and neither can patch names or descriptions, which Manager shows as plain text.
- Write tags as tags. Escaped markup such as `&lt;b&gt;` shows as the literal text.

## Add the settings screen

The settings host is an internal patch that adds your bundle's screen to one app:

```kotlin
val appSettings = settingsHost("example") {
    compatibleWith(EXAMPLE_APP)
    brand(exampleBrand)

    install {
        settingsRow.after { openSettings() }
    }
}
```

Users need a way to open the screen. The usual place is a row in the app's own settings. Hook that row in `install`, which runs after every patch that added settings. `openSettings()` shows your settings over whatever screen is open, and works from any thread.

Some apps build their settings rows from an intent, such as an androidx `Preference` with an `<intent>` child. For those, mark the intent and redirect the call that starts it:

```kotlin
install {
    resources.editXml("xml", "settings_main") {
        findByTag("intent").first().opensSettings()
    }
    startsSettingsActivity.redirectTo(Reseam.startActivity)
}
```

`Reseam.startActivity` opens your settings for the marked intent, and starts any other intent as usual.

If your extensions read strings that no setting uses, add their table with `strings(ExampleStrings)`, so it ships with the app. The tables your settings use ship anyway.

### The about card

The screen opens with an about card: your brand's logo and name, your bundle's version, the engine version, and the `license` from your [`manifest.toml`](16_bundles.md#manifesttoml). `about` adds ways to reach you:

```kotlin
about(
    About(
        links = listOf(Link(Verbatim("Discord"), "https://example.com/discord", discordIcon)),
        actions = listOf(Link(ExampleStrings.donate, "https://example.com/donate", heartIcon)),
    )
)
```

`links` become a row of icon buttons under your name, and `actions` become buttons below it. Each `Link` needs an `https://` address. An icon button shows no label, but screen readers read it aloud.

The screen ends with Export settings and Import settings. An export is a JSON file with your bundle's name and the value of every toggle, text, and choice setting. Folder settings stay out, since their permission belongs to the install. Import applies the values that match a setting on the screen, and leaves the rest, so a file from another app of your bundle only changes the settings both share.

## Show settings on the screen

A patch adds its settings to the screen with `settings(host, section(...))`:

```kotlin
val hideAds = patch(ExamplePatchStrings.hideAds) {
    compatibleWith(EXAMPLE_APP)
    settings(appSettings, section(ExampleStrings.sectionAds, AppSettings.hideAds))

    execute {
        gate(AppSettings.hideAds) {
            showAd.alwaysReturn()
        }
    }
}
```

`settings` makes the host a dependency of the patch. Only the settings of patches that ran appear, so the screen never shows a switch that does nothing.

`gate` makes the hooks inside it take effect only while the toggle is on. See [Gates](7_changing_code.md#gates).

When a setting depends on an option, register it from a block. The block runs after `execute`, so it can read options:

```kotlin
settings(appSettings) {
    listOf(section(ExampleStrings.sectionName, AppSettings.appName))
}
```

A choice whose title the user typed, such as a custom app name from an option, uses `Verbatim(name)` instead of a string, and shows as it is in every language.

## Pages

Group sections behind a row on the screen with a page:

```kotlin
val mediaPage = SettingsPage(
    "media",
    ExampleStrings.pageMedia,
    order = 10,
    summary = ExampleStrings.pageMediaSummary,
    icon = media,
)

settings(appSettings, section(mediaPage, ExampleStrings.sectionPlayback, AppSettings.autoplay))
```

- A page can have a `parent` page.
- Lower `order` comes first among siblings.
- Sections with the same page and title merge, so several patches can add to one section.
- Pages without settings from patches that ran are left out.

Patches add their sections in the order they run. To fix where each setting appears regardless of that order, list the sections in the host's `layout`:

```kotlin
val appSettings = settingsHost("example") {
    layout(
        section(mediaPage, AppSettings.autoplay, AppSettings.quality),
        section(mediaPage, ExampleStrings.sectionAdvanced, AppSettings.codec),
        stored(AppSettings.queueOrder),
    )
}
```

A section without a title continues the section before it.

A `stored` setting keeps its value, and settings export carries it, but it has no row. Use it for values a patch's own dialog changes.

A page's row comes before the settings of its parent page. To show it among them, place it after a setting:

```kotlin
val appSettings = settingsHost("example") {
    place(downloadsPage, after = AppSettings.autoplay, heading = ExampleStrings.sectionDownloads)
}
```

The row follows the setting's row while that setting shows. With a `heading`, it starts a new group under that heading.

## Brand

A brand decides how your settings screen and your branded components look. The screen follows the system's light or dark theme, and before Android 10, which has none, the app's own, until the user picks one with the button at the end of its top bar. A brand has a color scheme for each:

```kotlin
private object Files

val exampleBrand = Brand(
    name = "Example",
    light = ColorScheme(primary = Color.hex("#296A48"), onPrimary = Color.hex("#FFFFFF"), ...),
    dark = ColorScheme(primary = Color.hex("#A2EBBD"), onPrimary = Color.hex("#000000"), ...),
    logo = Logo(Vector.svg(Files::class.java.getResource("/brand/logo.svg")!!.readText())),
    font = Font(Files::class.java.getResource("/brand/Inter.ttf")!!.readBytes()),
    typography = Typography(titleLarge = TextStyle(size = 22f, lineHeight = 28f, weight = 600)),
)
```

- `ColorScheme` takes the 20 Material 3 roles the screen paints with. Every role is required.
- `Logo(light, dark)` takes a vector for each theme, or one for both.
- `Font` takes a TrueType or OpenType file. A variable font with a `wght` axis renders every weight the type roles use.
- `Typography` defaults to Material 3's type scale. Override the roles you want different.

A host without a brand uses `Brand.DEFAULT`: Material 3's baseline colors, in the system font. Put your brand's files in your patch module's `src/main/resources`, which ends up in the patch jar.

## Icons

Settings and pages can start with an icon. Any [Lucide](https://lucide.dev/icons) icon works by name, since the patch API ships the whole set:

```kotlin
val hideAds by toggle(default = true, icon = Vector.lucide("megaphone-off"))
```

Use the name Lucide lists today: old aliases such as `trash-2` fail the build. Lucide icons cost your bundle nothing.

For your own icons, `Vector.svg` reads an SVG from your patch module's resources:

```kotlin
private val ads = Vector.svg(Files::class.java.getResource("/icons/megaphone.svg")!!.readText())
```

`Vector.svg` reads an SVG when your patches load: paths and basic shapes, with fills and strokes. The SVG needs a `viewBox`, or a `width` and `height`. `currentColor` draws in the screen's icon color. Transforms, `style` attributes, gradients, and text fail the build.

To show icons on only some rows, a patch lists the pages and settings that keep theirs:

```kotlin
execute {
    appSettings.showIconsOf(setOf("media", AppSettings.hideAds.key))
}
```

`showIconsOf` takes page ids and setting keys, and every other icon is left out of this run's screen. Without a call every icon shows. When several patches call it, the last one to run decides.

## Read settings in code blocks

Besides gates, code blocks can branch on a toggle with `whenEnabled`, and read any setting with `valueOf`:

```kotlin
configureDownloads.after {
    thisObject.set(downloadFolder, valueOf(AppSettings.downloadFolder))
}
```

Code that runs before the app's `Application.onCreate`, such as a content provider, reads the defaults.

## Actions

An action is a row that does something when the user taps it, such as copying a login or opening a page. Declare it like the other settings, show it in a section, and give it code with `onAction` in the same patch:

```kotlin
object AppSettings : SettingsGroup(ExampleStrings) {
    val exportLogin by action(key = "app.export_login")
}

val loginTransfer = patch(ExamplePatchStrings.exportLogin) {
    settings(appSettings, section(ExampleStrings.sectionAccount, AppSettings.exportLogin))
    execute {
        onAction(AppSettings.exportLogin) {
            Login.copy(activity)
        }
    }
}
```

The block runs in the app on every tap. `activity` is the screen your settings are shown over, for a dialog, a toast or an intent. The work itself usually belongs in an [extension](10_extensions.md) that the block calls.

Things to know:

- Every action your settings show needs code from a patch of the same run. The settings host fails, and names the action, when one has none.
- An action stores nothing, so settings export and import skip it, and extensions have no value to read for it.

## Rows that react

A row can follow other settings or the app's state. It can grey out while a toggle is off, show a summary your code computes, check text before it is stored, or run code after a change. Call these in `execute`, or in the `settings` block, of a patch that shows the setting:

```kotlin
execute {
    enabledWhile(AppSettings.adDelay, AppSettings.hideAds)
    onSummary(AppSettings.cacheSize) { returnValue(Cache.describe(context, value)) }
    onValidate(AppSettings.serverUrl) { returnValue(Server.problemWith(value)) }
    onEnabled(AppSettings.quality) { returnValue(Device.supportsHdr(context)) }
    onChange(AppSettings.uploadSegments) { Guidelines.show(activity, previous) }
}
```

- `enabledWhile(setting, toggle)` greys out the row and stops it taking changes while `toggle` is off.
- `onSummary` computes the line under the title. `value` is the setting's value as text, and `context` the screen. Returning null shows the declared summary.
- `onValidate` checks text the user entered before it is stored. Returning a message keeps the old value, shows the message, and opens the editor again with the text.
- `onEnabled` decides in the app whether the row takes changes, for conditions `enabledWhile` can't express, such as several toggles or the Android version.
- `onChange` runs after the user changes the setting on the screen, with `previous`, the value before the change, as text.

Things to know:

- `onSummary` and `onEnabled` run when the row shows and again after any setting changes, because what they read may depend on other settings. Keep their code fast, and move slow work into an extension that remembers its result.
- With both `enabledWhile` and `onEnabled`, the row takes changes only when both allow it.
- Changes made by code, or by importing settings, don't run `onChange`.
- `onValidate` takes a text setting. The other kinds can't hold invalid input.

## Read settings in extensions

[Extensions](10_extensions.md) compile against objects the build generates from your settings module. They have the same names, with a value for each setting that the extension reads and writes while the app runs:

```kotlin
import app.example.settings.AppSettings
import app.example.settings.ExampleStrings

if (AppSettings.hideAds.value) hide(view)
val style = AppSettings.feedStyle.value
AppSettings.feedStyle.reset()
Toast.makeText(context, ExampleStrings.downloadSaved(folder), Toast.LENGTH_SHORT).show()
```

- A folder setting's `pick(activity) { ... }` opens the system folder picker, stores the folder the user picks, then runs the block. A cancelled pick runs nothing.
- A file setting's `pick(activity) { ... }` does the same with the system file picker, for the setting's MIME types. For a one-off file, such as an export, `Reseam.openFile` and `Reseam.saveFile` open the pickers without a setting.
- Actions have no object here; their code comes from [`onAction`](#actions).
- A choice reads as the stored value while the setting still lists it, and as the default otherwise.
- Before the app runtime has started, values read as their defaults.
- Strings without arguments are properties. The others are functions named after the string, taking its `xliff:g` arguments in order.

[API: App runtime](reference_runtime.md#settings-in-extensions) lists every member.

## Branded components

Extensions can build their own screens and dialogs in your brand:

```kotlin
val ui = Reseam.bundle("my-patches").ui(activity)
ui.dialog()
    .title(ExampleStrings.downloadFinished)
    .message(ExampleStrings.downloadSavedToDownloads)
    .dismissAction(ExampleStrings.close)
    .action(ExampleStrings.open) { open(file) }
    .show()
```

`Ui` builds text in your brand's type roles, buttons, list rows in groups, switches, search bars, text fields, top bars, and dialogs, all in the screen's current light or dark theme. See [Branded components](reference_runtime.md#branded-components).

## React Native apps

[Hermes gates](14_hermes.md#gate-on-a-setting) read settings through a React Native module. Add it from your host's `install`, at a hook where you hold the app's list of React Native packages:

```kotlin
getPackages.after { addReactPackage(result) }
```

It is added once, however many bundles call it. Apps built with R8 can rename React Native's package types, so the engine finds the app's package interface itself.

To register native modules of your own, ship a package class in an extension and pass its `addTo`:

```kotlin
object ExamplePackage {
    fun createNativeModules(context: ReactApplicationContext): List<NativeModule> =
        listOf(ExampleModule(context))

    fun createViewManagers(context: ReactApplicationContext): List<Any> = emptyList()

    @Export
    @JvmStatic
    fun addTo(packages: MutableList<Any?>) {
        if (packages.none { it === this }) packages.add(this)
    }
}
```

```kotlin
getPackages.after { addReactPackage(result, ExamplePackage.addTo) }
```

The class doesn't extend or implement a React Native package type, since R8 may have renamed it. The engine makes it implement the app's package interface, so declare `createNativeModules` and `createViewManagers` with the interface's signatures. Its modules register the way the runtime's own do.

Next: [Reading obfuscated objects](12_bindings.md).
