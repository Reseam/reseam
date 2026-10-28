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

A setting's title is the string named after its property in snake case: `hideAds` reads `hide_ads`. Its summary is the same name with `_summary`, when there is one. A missing title fails the build. The generated `ExampleStrings` has one member per string, for choices, [sections](#show-settings-on-the-screen), and [pages](#pages).

There are four kinds of setting, each with a default and an optional [icon](#icons):

| Kind | Holds |
|---|---|
| `toggle(default)` | on or off |
| `text(default, multiline = false)` | text the user types. A `multiline` text holds one entry per line. |
| `folder(default)` | a folder the user picks, with permission to read and write it |
| `choice(default, choices)` | one of several values |

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

## Translations

Put translations next to the default strings, in Android's folder names: `values-de/strings.xml`, `values-pt-rBR/strings.xml`, `values-b+sr+Latn/strings.xml`. The app shows each string in its current language, including Android's per-app language setting, and falls back to the default when a translation lacks it.

- Escape apostrophes and quotes as `\'` and `\"`.
- Wrap each argument of a string in `xliff:g`, whose `id` names it: `Saved to <xliff:g id="folder">%1$s</xliff:g>`. `%s` takes text, `%d` a whole number, and `%f` a decimal number.
- `<plurals>` picks its form by an integer argument with the id `count`.

A translation must keep the default's arguments. One that adds, drops, or changes the type of an argument fails the build, and so does a translation of a string the default lacks.

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

The screen opens with an about card: your brand's logo and name, your bundle's version, the engine version, and the `license` from your [`manifest.toml`](15_bundles.md#manifesttoml). `about` adds ways to reach you:

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
val hideAds = patch("Hide ads") {
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

## Brand

A brand decides how your settings screen and your branded components look. The screen follows the system's light or dark theme, and before Android 10, which has none, the app's own, so a brand has a color scheme for each:

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

Settings and pages can start with an icon:

```kotlin
private val ads = Vector.svg(Files::class.java.getResource("/icons/megaphone.svg")!!.readText())

val hideAds by toggle(default = true, icon = ads)
```

`Vector.svg` reads an SVG when your patches load: paths and basic shapes, with fills and strokes. The SVG needs a `viewBox`, or a `width` and `height`. `currentColor` draws in the screen's icon color, so line icons such as [Lucide](https://lucide.dev) work as they are. Transforms, `style` attributes, gradients, and text fail the build.

## Read settings in code blocks

Besides gates, code blocks can branch on a toggle with `whenEnabled`, and read any setting with `valueOf`:

```kotlin
configureDownloads.after {
    thisObject.set(downloadFolder, valueOf(AppSettings.downloadFolder))
}
```

Code that runs before the app's `Application.onCreate`, such as a content provider, reads the defaults.

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

Next: [Reading obfuscated objects](12_bindings.md).
