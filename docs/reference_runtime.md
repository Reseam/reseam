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
| `default: T` | Its default. |
| `value: T` | Its current value. Set it to change the setting. |
| `reset()` | Sets it back to the default. |

### `ToggleValue`

A toggle's value, as `Boolean`. A toggle covered by another one that is on reads as `true`.

### `TextValue`

A text setting's value, as `String`.

### `FolderValue`

A folder setting's value, as `String`, the folder's address. It also has:

`pick(activity: Activity, onPicked: Runnable)` opens the system folder picker over `activity`. When the user picks a folder, it becomes the value, with permission to read and write it, and `onPicked` runs on the main thread. A cancelled pick runs nothing.

### `ChoiceValue`

A choice setting's value, as `String`. A stored value that the setting no longer lists reads as the default.

## Strings in extensions

The build generates an object for each of your string tables, with the same name as the patch side's table. A string without arguments is a `String` property. A string with arguments is a function named after it, taking the `xliff:g` arguments in order. A plural takes its `count` first.

```kotlin
Toast.makeText(context, ExampleStrings.downloadSaved(folder), Toast.LENGTH_SHORT).show()
val label = ExampleStrings.itemsHidden(count)
```

Each string shows in the app's current language, including Android's per-app language setting, and falls back to the default text when a translation lacks it. Reading a string before the app runtime has started fails.

### `StringTable`

What the generated objects read through. You rarely use it directly.

| Member | Description |
|---|---|
| `StringTable(directory: String)` | The table stored in the app at `directory`. |
| `get(name: String): String` | A string without arguments, written `table["name"]`. |
| `format(name: String, vararg args: Any?): String` | A string with arguments. |
| `plural(name: String, count: Int, vararg args: Any?): String` | A plural, in the form the current language uses for `count`. |

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
| `startActivity(context: Context, intent: Intent)` | Opens settings for an intent that [`opensSettings`](reference_settings.md#the-app-runtime-in-code-blocks) set up, and starts any other intent as usual. |

### `ReseamBundle`

One bundle in the app, from `Reseam.bundle(name)`.

| Member | Description |
|---|---|
| `name: String` | The bundle's name. |
| `settings: Settings` | Its stored settings. |
| `ui(context: Context): Ui` | Builds [branded components](#branded-components) in the bundle's brand and the settings screen's current light or dark theme. |
| `openSettings()` | Opens its settings over the current screen. It works from any thread. |

### `Settings`

A bundle's stored settings, by key. The generated setting values are usually simpler.

| Member | Description |
|---|---|
| `getBoolean(key: String, defaultValue: Boolean): Boolean` | A toggle. A covered toggle reads as on while a toggle that covers it is on. |
| `getString(key: String, defaultValue: String): String` | A text or folder setting. |
| `getChoice(key: String): String` | A choice setting. A stored value it no longer lists reads as its default. |
| `setBoolean(key: String, value: Boolean)`, `setString(key: String, value: String)` | Changes a setting. |
| `addOnChangeListener(listener: OnChangeListener)` | Calls `listener.onChanged(key)` on the main thread after every change, until removed. A null key means every value was cleared. |
| `removeOnChangeListener(listener: OnChangeListener)` | Stops calling the listener. |

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
| `itemGroup(): ItemGroup` | A group of list rows with shared rounded corners. |
| `sectionLabel(label: CharSequence?): TextView` | A heading above a group. |
| `switchView(): SwitchView` | A switch. |
| `searchBar(hint: CharSequence?): SearchBar` | A search field. |
| `textField(label: CharSequence?): TextField` | An outlined text field. |
| `topBar(title: CharSequence?, back: Runnable): TopBar` | A screen's top bar with a back button. |
| `dialog(): BrandDialog.Builder` | A dialog. |
| `shape(color: Int, radiusDp: Float)`, `shape(color: Int, topDp: Float, bottomDp: Float): GradientDrawable` | A rounded background. |
| `ripple(content: Drawable?, topDp: Float, bottomDp: Float, stateColor: Int): RippleDrawable` | A touch ripple over a background. |

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
