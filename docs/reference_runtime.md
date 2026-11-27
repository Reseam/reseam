---
title: "API: App runtime"
description: What your extensions call inside the patched app: settings, strings, and branded components.
---

# API: App runtime

The app runtime is code the engine adds to the patched app, the first time a patch uses it. Your [extensions](10_extensions.md) compile against it and call it while the app runs. Your bundle never ships it, and any number of bundles share one copy in an app.

Everything here is in `app.reseam.runtime`, and runs on the phone, unlike the rest of the API, which runs while patching.

## Exports

### `Export`

Marks an extension class, method, constructor, or field that patches refer to. The build generates a binding for every class that has exports, so patches can call it. See [Export what patches call](10_extensions.md#export-what-patches-call).

| Member | Description |
|---|---|
| `@Export` | Exports the member under its own name. |
| `@Export(name = "…")` | Exports it under another binding name, which two overloads need. |

## Settings in extensions

The build generates an object for each of your [settings groups](reference_settings.md#settingsgroup), in the same package and with the same name, for your extensions to compile against. Each property is a value of the matching class below.

```kotlin
import app.example.settings.AppSettings

if (AppSettings.hideAds.value) hide(view)
AppSettings.feedStyle.value = "cards"
AppSettings.feedStyle.reset()
```

Before the app runtime has started, in the app's `Application.onCreate`, every value reads as its default, and writing fails.

### `SettingValue`

The base of every setting value.

| Member | Description |
|---|---|
| `bundle: String` | Your bundle's name. |
| `key: String` | The setting's key. |
| `default: T` | The value until the user changes it: the default a patch gave the setting with `copy`, or the one it was declared with. |
| `value: T` | Its current value. Set it to change the setting. |
| `reset()` | Sets it back to the default. |

### `ToggleValue`

A toggle's value, as `Boolean`. A toggle covered by another one that is on reads as `true`.

### `TextValue`

A text setting's value, as `String`.

### `NumberValue`

A number setting's value, as `Double`. It is always on the setting's slider. A stored number off the slider, such as one from an older build, reads as the default. Setting a number off the slider fails.

### `RangeValue`

A range setting's value, as a `NumberRange`: `start: Double` and `end: Double`, with `start` no later than `end`. Both are always on the setting's slider, as with `NumberValue`.

### `ColorValue`

A color setting's value, as an ARGB `Int`. A setting without opacity always reads as opaque.

### `FolderValue`

A folder setting's value, as `String`, the folder's address. It also has:

`pick(activity: Activity, onPicked: Runnable)` opens the system folder picker over `activity`. When the user picks a folder, it becomes the value, with permission to read and write it, and `onPicked` runs on the main thread. A cancelled pick runs nothing.

### `FileValue`

A file setting's value, as `String`, the file's `content://` address. It also has:

`pick(activity: Activity, onPicked: Runnable)` opens the system file picker over `activity`, for the setting's MIME types. When the user picks a file, it becomes the value, with permission to read it, and `onPicked` runs on the main thread. A cancelled pick runs nothing.

### `ChoiceValue`

A choice setting's value, as `String`. A stored value that the setting no longer lists reads as the default.

## Strings in extensions

The build generates an object for each of your string tables, with the same name as the patch side's table. A string without arguments is a `String` property. A string with arguments is a function named after it, taking the `xliff:g` arguments in order. A plural takes its `count` first.

A [styled string](11_settings.md#styled-strings) is a `CharSequence` instead, with its styling, so a `TextView` shows it bold or italic. Text arguments of a styled string show as they are, even when they contain `<` or `&`.

```kotlin
Toast.makeText(context, ExampleStrings.downloadSaved(folder), Toast.LENGTH_SHORT).show()
val label = ExampleStrings.itemsHidden(count)
```

Each string shows in the language the user picked in Reseam's settings, else in the first of the user's languages that translates it, with Android's per-app language first. The app doesn't need a translation of its own in that language. A string no translation covers shows its default text, which is English. Reading a string before the app runtime has started fails.

### `StringTable`

What the generated objects read through. You rarely use it directly.

| Member | Description |
|---|---|
| `StringTable(directory: String)` | The table stored in the app at `directory`. |
| `get(name: String): String` | A string without arguments, written `table["name"]`. |
| `format(name: String, vararg args: Any?): String` | A string with arguments. |
| `styled(name: String): CharSequence` | A [styled string](11_settings.md#styled-strings) without arguments, with its styling. |
| `formatStyled(name: String, vararg args: Any?): CharSequence` | A styled string with arguments. Text arguments show as they are, not as markup. |
| `plural(name: String, count: Int, vararg args: Any?): String` | A plural, in the form its text's language uses for `count`. |
| `layoutDirection: Int` | `View.LAYOUT_DIRECTION_RTL` when the language the table shows reads right to left, else `View.LAYOUT_DIRECTION_LTR`. Set it on a screen you build from the table's text. |

## Reaching the runtime

### `Reseam`

The app runtime's entry point.

| Member | Description |
|---|---|
| `isStarted: Boolean` | Whether the runtime has started. The settings host starts it first in the app's `Application.onCreate`. |
| `bundle(name: String): ReseamBundle` | A bundle's settings and components. Fails before the runtime has started, or when the app has no settings for that bundle. |
| `init(context: Context)` | Starts the runtime. The settings host calls it for you, and later calls do nothing. |
| `isEnabled(bundle: String, key: String, defaultValue: Boolean): Boolean` | A toggle's value by key. Generated setting values call this. |
| `getString(bundle: String, key: String, defaultValue: String): String` | A text setting's value by key. |
| `getChoice(bundle: String, key: String, defaultValue: String): String` | A choice setting's value by key. |
| `openSettings(bundle: String)` | Opens a bundle's settings over the current screen. |
| `runAction(bundle: String, key: String, activity: Activity)` | Runs the code patches gave an action with [`onAction`](reference_settings.md#onaction). The settings screen calls it when the action is tapped. |
| `settingSummary(bundle: String, key: String, value: String, context: Context): CharSequence?` | The summary patches compute with [`onSummary`](reference_settings.md#onsummary), or null for the declared one. The settings screen calls it. |
| `settingEnabled(bundle: String, key: String, context: Context): Boolean` | Whether code from [`onEnabled`](reference_settings.md#onenabled) lets the row take changes. The settings screen calls it. |
| `validateSetting(bundle: String, key: String, value: String): CharSequence?` | The error message code from [`onValidate`](reference_settings.md#onvalidate) gives for entered text, or null to store it. The settings screen calls it. |
| `settingChanged(bundle: String, key: String, previous: String, activity: Activity)` | Runs the code patches gave a setting with [`onChange`](reference_settings.md#onchange). The settings screen calls it after the user changes the setting. |
| `openFile(activity: Activity, mimeTypes: List<String>, onPicked: Consumer<Uri>)` | Lets the user pick a file of one of `mimeTypes`, for a file an extension reads once. `onPicked` gets it, readable while the app runs. A cancelled pick runs nothing. |
| `saveFile(activity: Activity, name: String, mimeType: String, onCreated: Consumer<Uri>)` | Lets the user choose where to save a new file, suggesting `name`, such as for an export. `onCreated` gets the new, empty file to write with `contentResolver.openOutputStream`. A cancelled pick runs nothing. |
| `startActivity(context: Context, intent: Intent)` | Opens settings for an intent that [`opensSettings`](reference_settings.md#the-app-runtime-in-code-blocks) set up, and starts any other intent as usual. |

### `ReseamBundle`

One bundle in the app, from `Reseam.bundle(name)`.

| Member | Description |
|---|---|
| `name: String` | The bundle's name. |
| `settings: Settings` | Its stored settings. |
| `ui(context: Context): Ui` | Builds [branded components](#branded-components) in the bundle's brand and the settings screen's current light or dark theme. |
| `openSettings()` | Opens its settings over the current screen. It works from any thread. |
| `version: String?` | The bundle's version, or null when it was built without one. |
| `refreshRows()` | Has open settings screens read again the summaries and states patch code computes, for a value that changes while no setting does, such as one measured in the background. Call it on the main thread. |

### `Settings`

A bundle's stored settings, by key. The generated setting values are usually simpler.

| Member | Description |
|---|---|
| `getBoolean(key: String, defaultValue: Boolean): Boolean` | A toggle. A covered toggle reads as on while a toggle that covers it is on. |
| `getString(key: String, defaultValue: String): String` | A text, folder or file setting. |
| `getChoice(key: String): String` | A choice setting. A stored value it no longer lists reads as its default. |
| `getChoice(key: String, defaultValue: String): String` | The same, for a choice the app may not show. A choice it doesn't show reads as `defaultValue`. |
| `getNumber(key: String, defaultValue: Double): Double` | A number setting. A stored value off its slider reads as its default. |
| `getRange(key: String, defaultValue: NumberRange): NumberRange` | A range setting, read like a number. |
| `getColor(key: String, defaultValue: Int): Int` | A color setting, as ARGB. One without opacity reads as opaque. |
| `setBoolean(key: String, value: Boolean)`, `setString(key: String, value: String)` | Changes a setting. |
| `setNumber(key: String, value: Double)`, `setRange(key: String, value: NumberRange)` | Changes a number or range setting. A value off the slider fails. |
| `setColor(key: String, value: Int)` | Changes a color setting to an ARGB color. |
| `addOnChangeListener(listener: OnChangeListener)` | Calls `listener.onChanged(key)` on the main thread after every change, until removed. A null key means every value was cleared. |
| `removeOnChangeListener(listener: OnChangeListener)` | Stops calling the listener. |

A setting your settings screen shows starts at the default your patches gave it. `defaultValue` is for a setting no section shows, and such a choice always reads as `defaultValue`, because there is no list to check a stored value against.

## Branded components

Views your extensions build in your bundle's [brand](reference_settings.md#brand), for their own screens and dialogs:

```kotlin
val ui = Reseam.bundle("my-patches").ui(activity)
ui.dialog()
    .title(ExampleStrings.downloadFinished)
    .message(ExampleStrings.downloadSavedToDownloads)
    .dismissAction(ExampleStrings.close)
    .action(ExampleStrings.open) { open(file) }
    .show()
```

They follow Material 3, in the settings screen's current light or dark theme: the one the user picked, and until they pick, the system's, or before Android 10 the app's own.

### `Ui`

| Member | Description |
|---|---|
| `context: Context`, `theme: Theme` | What it builds with. |
| `colors: Colors` | The current theme's colors. |
| `type: Typography` | The brand's type roles. |
| `px(dp: Float): Int` | Converts dp to pixels. |
| `text(value: CharSequence?, style: TypeStyle, color: Int): TextView` | Text in a type role. |
| `icon(image: VectorImage, sizeDp: Int, tint: Int): ImageView` | An icon, ignored by screen readers. |
| `iconButton(image: VectorImage, tint: Int, description: CharSequence?, onClick: View.OnClickListener?): ImageView` | An icon button. Screen readers read `description`. |
| `button(label: CharSequence?, onClick: View.OnClickListener?): TextView` | A text button. |
| `tonalButton(label: CharSequence?, image: VectorImage, onClick: View.OnClickListener?): View` | A filled button with an icon. |
| `listItem(title: CharSequence?): ListItem` | A list row. |
| `buttonRow(buttons: List<View>): ButtonRow` | Buttons side by side in equal shares, stacked at full width when a label would not fit its share. |
| `itemGroup(): ItemGroup` | A group of list rows with shared rounded corners. |
| `sectionLabel(label: CharSequence?): TextView` | A heading above a group. |
| `switchView(): SwitchView` | A switch. |
| `searchBar(hint: CharSequence?): SearchBar` | A search field. |
| `listScreen(title: CharSequence?, searchHint: CharSequence?): ListScreen` | A full-screen list with a search field, over the activity the `Ui` was made for. |
| `textField(label: CharSequence?): TextField` | An outlined text field. |
| `topBar(title: CharSequence?, back: Runnable): TopBar` | A screen's top bar with a back button. |
| `dialog(): BrandDialog.Builder` | A dialog. |
| `shape(color: Int, radiusDp: Float)`, `shape(color: Int, topDp: Float, bottomDp: Float): GradientDrawable` | A rounded background. |
| `ripple(content: Drawable?, topDp: Float, bottomDp: Float, stateColor: Int): RippleDrawable` | A touch ripple over a background. |

### `ListScreen`

A full-screen list for your extension's own screens, such as a long list to search through. Its rows are [`ListItem`](#listitem)s, so each can have a supporting line, an icon, a trailing view and a tap action. Its search field keeps the rows whose title or supporting text contains the query, and the groups that still have one.

| Member | Description |
|---|---|
| `group(label: CharSequence?, items: List<ListItem>): ListScreen` | Adds the rows as one group, under `label` when it isn't null. An open screen shows it at once, so rows can arrive after loading. |
| `clear(): ListScreen` | Removes every group, such as a loading row once the data is in. |
| `onDismiss(listener: Runnable): ListScreen` | Runs `listener` once the screen closes. |
| `show()` | Opens the screen with an empty search. Back closes it. |

```kotlin
val ui = Reseam.bundle("my-patches").ui(activity)
ui.listScreen(ExampleStrings.flags, ExampleStrings.searchFlags)
    .group(ExampleStrings.overridden, overridden.map { flag ->
        ui.listItem(flag.name).supporting(flag.value).onClick { editFlag(ui, flag) }
    })
    .group(ExampleStrings.allFlags, flags.map { flag ->
        ui.listItem(flag.name).trailing(ui.switchView().apply { isChecked = flag.on })
    })
    .show()
```

### `ListItem`

A list row, a `View`. Each setter returns the row, so calls chain.

| Member | Description |
|---|---|
| `icon(image: VectorImage?)` | The leading icon. |
| `title(value: CharSequence?)` | The title. |
| `supporting(value: CharSequence?)` | The line under the title. |
| `trailing(view: View?)` | A view at the end, such as a switch. |
| `chevron()` | A trailing arrow, for a row that opens something. |
| `onClick(listener: View.OnClickListener?)` | Makes the row clickable. |
| `isEnabled: Boolean` | Set to false, the row fades and stops taking taps, and so does its trailing view. |

### `ButtonRow`

A `LinearLayout` of buttons, such as `tonalButton`s. It measures each button's label, so a long translation stacks the buttons instead of wrapping their text.

### `ItemGroup`

A `LinearLayout` of rows. `add(item: ListItem): ItemGroup` appends one, and rounds the group's first and last corners.

### `SwitchView`

A Material 3 switch, a `View` that is `Checkable`. `setOnCheckedChangeListener(listener)` calls `onCheckedChanged(view, checked)` when the user flips it.

### `SearchBar`

A search field, a `LinearLayout`. `onQueryChange(listener)` calls `onQueryChanged(query)` as the user types.

### `TextField`

An outlined text field, a `LinearLayout`.

| Member | Description |
|---|---|
| `field: EditText` | The editable text view inside. |
| `text: String` | The current text. |
| `text(value: CharSequence?): TextField` | Sets the text. |
| `multiline(lines: Int): TextField` | Lets the text wrap, showing this many lines. |
| `action(icon: VectorImage, description: CharSequence?, onClick: View.OnClickListener?): TextField` | An icon button at the end of the field. |
| `showAction(visible: Boolean)` | Shows or hides that button. |

### `TopBar`

A screen's top bar, with a back button that runs `back`.

| Member | Description |
|---|---|
| `action(icon: VectorImage, description: CharSequence?, onClick: View.OnClickListener?): TopBar` | An icon button at the end of the bar. Screen readers read `description`. |

### `BrandDialog.Builder`

What `ui.dialog()` returns. Each setter returns the builder.

| Member | Description |
|---|---|
| `icon(icon: VectorImage?)` | An icon above the title. |
| `title(title: CharSequence?)` | The title. |
| `message(message: CharSequence?)` | The body text. |
| `view(view: View?)` | A view of your own as the body. |
| `choices(choices: Array<out CharSequence?>?, selected: Int, onChoose: OnChooseListener?)` | A list of options, with `selected` marked. `onChoose` gets the picked index. |
| `action(label: CharSequence?, onClick: Runnable?)` | A button that closes the dialog, then runs `onClick`. Add the confirming action last. |
| `dismissAction(label: CharSequence?)` | A button that only closes the dialog. |
| `onDismiss(onDismiss: Runnable?)` | Runs when the dialog closes, however it closes. |
| `show(): Dialog` | Builds and shows it. |

### `Theme`

The brand in the current light or dark theme.

| Member | Description |
|---|---|
| `brand: Brand`, `dark: Boolean` | The brand, and whether the theme is dark. |
| `logo: VectorImage?` | The brand's logo for this theme. |
| `typeface(weight: Int): Typeface` | The brand's font at a weight. |
| `style(view: TextView, style: TypeStyle)` | Sets a text view in a type role. |

### `Colors`

The current theme's colors, as Android color ints, one per [Material 3 role](reference_settings.md#colorscheme): `primary`, `onPrimary`, `primaryContainer`, `onPrimaryContainer`, `secondaryContainer`, `onSecondaryContainer`, `surface`, `surfaceContainerLow`, `surfaceContainer`, `surfaceContainerHigh`, `surfaceContainerHighest`, `onSurface`, `onSurfaceVariant`, `outline`, `outlineVariant`, `error`, `onError`, `errorContainer`, `onErrorContainer`, and `scrim`.

### `Typography` and `TypeStyle`

The brand's type roles: `headlineSmall`, `titleLarge`, `titleMedium`, `bodyLarge`, `bodyMedium`, `labelLarge`, and `labelMedium`. Each `TypeStyle` has `size`, `lineHeight`, and `tracking` in sp, and a `weight`. Apply one with `ui.text` or `theme.style`.

### `VectorImage`

A vector image from your brand or a setting's icon, drawn at any size. Draw one with `ui.icon`.

### `Icons`

Line icons the runtime's own screens use, which your components can use too: `ARROW_LEFT`, `CHEVRON_RIGHT`, `SEARCH`, `CLOSE`, `CHECK`, `RESET`, `SUN`, `MOON`, `UPLOAD`, and `DOWNLOAD`.

## Used by the runtime itself

These are public so the runtime's parts can reach each other. Extensions don't need them: `Tokens` and `Motion` (spacing, sizes, and animations of the runtime's screens), `DocumentRequest` (the screen that runs the system file pickers), and `ReseamReactPackage` (the React Native module Hermes gates read settings through, which `addReactPackage` adds).
