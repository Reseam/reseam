# API reference

Every public symbol of the patch SDK, by package. Types are accepted as descriptors (`Ljava/lang/String;`), dotted names (`java.lang.String`), or `Type` constants everywhere a `type: String` parameter appears.

## `app.reseam.patch`

### Declaring patches

| Symbol | Description |
|---|---|
| `patch(name: String) { }` | A patch users see. Returns `ReseamPatch`. |
| `patch { }` | An internal patch: hidden, runs only as a dependency. |
| `"pkg"("1.0", "1.1")` | `String.invoke`: a `CompatiblePackage` with versions. |
| `CompatiblePackage(name, versions)` | A package the patch applies to; empty `versions` means all. |
| `ReseamPatch` | The interface the engine reads: `name`, `hidden`, `description`, `dependencies`, `compatibleWith`, `enabled`, `options`, `execute(ctx)`, `afterDependents(ctx)`. |

Inside `patch { }` (`PatchBuilder`):

| Member | Description |
|---|---|
| `description(text)` | Shown to users; indentation trimmed. |
| `compatibleWith(vararg packageNames)` | Packages, any version. |
| `compatibleWith(vararg packages: CompatiblePackage)` | Packages with versions. |
| `dependsOn(vararg patches)` | Patches that run first. |
| `enabledByDefault(Boolean)` | Initial selection state. Default `true`. |
| `hidden()` | Keep a named patch off the lists. |
| `stringOption(key, title, description, default, validValues, required)` | Declares and registers a `StringOption`. |
| `boolOption(key, title, description, default, required)` | `BoolOption`. |
| `intOption(...)` | `IntOption` (`Long` values). |
| `floatOption(...)` | `FloatOption` (`Double` values). |
| `stringListOption(...)` | `StringListOption`. |
| `pathOption(key, title, description, required)` | `PathOption`; a directory the user picks. |
| `settings(host, vararg sections)` | Registers settings; `host` becomes a dependency. |
| `execute { }` | The body. Receiver `PatchRuntime`. |
| `afterDependents { }` | Runs after every dependent finished. |

### Options

| Symbol | Description |
|---|---|
| `Option<T>` | `key`, `title`, `description`, `required`, `kind: OptionKind`, `default`, `validValues`. |
| `OptionKind` | `STRING`, `BOOL`, `INT`, `FLOAT`, `STRING_LIST`, `PATH`. |
| `options[option]` | The value with defaults applied; throws when absent. |
| `options.getOrNull(option)` | The value or null. |
| `OptionPath` | `path`, `listContents()`, `readFile(relativePath)`. |

### Targets

| Symbol | Description |
|---|---|
| `method(label) { MethodQuery }` | The one method matching the query. `MethodTarget`. |
| `methods(label) { MethodQuery }` | Every matching method, best ranked first. `MethodsTarget`. |
| `klass(name)` | A class by name. `ClassTarget`. |
| `klass(label) { ClassQuery }` | The one class matching the query. |
| `ClassTarget.method(name) { MethodQuery }` | A method of the class by name, narrowed by the block. |
| `ClassTarget.methods(label) { }` | Matching methods of the class. |
| `ClassTarget.field(name)` | A field by name. `FieldTarget`. |
| `ClassTarget.fieldOfType(type)` | The one instance field of that type. |
| `field(owner, name, type)` | A field reference without lookup. |
| `methodTarget(label) { PatchRuntime.() -> Method }` | A method resolved by hand. |
| `classTarget(label) { PatchRuntime.() -> DexClass }` | A class resolved by hand. |
| `fieldTarget(label) { PatchRuntime.() -> FieldRef }` | A field resolved by hand. |
| `appEntry` | `onCreate()` of the manifest's `Application` class, added if missing. |
| `Target.explain()` | The `MatchReport`: `name`, `winner`, `considered`, `reasons`, `nearMisses`. |

`MethodTarget`: `method`, `owner`, `name`, `proto`, `returnType`, `parameterTypes`, `descriptor`, `ref`. `MethodsTarget`: `all`, `forEach { }`, `single { }`. `ClassTarget`: `classDef`, `descriptor`. `FieldTarget`: `ref`, `owner`, `name`, `type`.

`MethodQuery`: `name`, `strings`, `literals`, `returns`, `params`, `param(index, type)`, `hasParam`, `paramCount`, `flags`, `inClass`, `calls`, `calledBy`, `callsMethod { MethodRef }`, `opcode`, `rankBy(label) { MethodRankScope }`, `first()`.

`ClassQuery`: `strings`, `hasInstanceField`, `extends`, `implements`, `rankBy(label) { ClassRankScope }`, `first()`.

`RankScope`: `type`, `methods(proto)`, `zeroArgListGetters()`. `MethodRankScope` adds `method`, `paramCount`, `callSitesFollowedByCast(type, lookAhead)`. `ClassRankScope` adds `classDef`.

### Points

| Symbol | Description |
|---|---|
| `MethodTarget.point(label) { PointMatch }` | The first instruction matching; a sequence ends at its last step. `PointTarget`. |
| `PointTarget.previous { }` | Nearest earlier match, one step. |
| `PointTarget.next { }` | Nearest later match, may be a sequence. |
| `PointTarget.captureAs(name, type?)` | Records the register written here for `capture(name)`. |
| `PointTarget.callee(label?)` | The invoked method as a `MethodTarget`. |
| `PointTarget.field(label?)` | The accessed field as a `FieldTarget`. |
| `PointTarget.index`, `.instruction`, `.method` | The resolved position and its method target. |

`PointMatch`: `opcode`, `string`, `stringContains`, `literal`, `type`, `checkCast`, `newInstance`, `invoke(opcodes) { MethodRefMatch }`, `invokeStatic`, `invokeVirtual`, `invokeInterface`, `invokeDirect`, `calls(target)`, `field { FieldRefMatch }`, `resultOf(returns?)`, `where { Instruction }`, `then(within) { }`.

`MethodRefMatch`: `owner`, `name`, `returns`, `params`, `hasParam`, `paramCount`. `FieldRefMatch`: `owner`, `name`, `type`.

### Changing methods

| Symbol | Description |
|---|---|
| `MethodTarget.before { CodeScope }` | Emit at entry. |
| `MethodTarget.after { CodeScope }` | Emit before every return; `capture("result")` is the return value. Referenced parameters and receiver are saved at entry in dedicated locals. |
| `MethodTarget.replace { CodeScope }` | Replace the body. |
| `PointTarget.before { }`, `.after { }` | Emit around the instruction. |
| `MethodTarget.alwaysReturn()`, `(Boolean)`, `(Int)`, `(Long)`, `(String)`, `alwaysReturnNull()` | Replace the body with a constant return. |
| `MethodTarget.replaceAllStrings(old, new)`, `replaceAllLiterals(old, new)` | Rewrite constants; returns the count. |

`CodeScope`: `thisObject`, `param(i)`, `paramOfType(type)`, `lastParam`, `capture(name)`, `int`, `long`, `bool`, `string`, `nullObject`, `enumValue(type, name)`, `staticField(FieldTarget | FieldRef)`, `newInstance(type, ctorProto, args)`, `call(ExtMethod | MethodTarget, args)`, `callStatic(owner, name, proto, args)`, `whenTrue`, `whenFalse`, `whenNull`, `whenNotNull`, `whenEqual`, `whenNotEqual` (each returns `Otherwise` with `otherwise { }`), `returnVoid`, `returnValue`, `returnTrue`, `returnFalse`, `returnNull`.

Inside `MethodTarget.after`, `param(i)`, `paramOfType(type)`, `lastParam`, and `thisObject` refer to entry snapshots, even if the method body reuses their incoming registers. Assigning to a snapshot changes the saved local, independently of `capture("result")`. Object snapshots preserve references, not object state. Point hooks read values at their instruction and do not take entry snapshots.

Temporary registers are reused after their last use across the block's control flow. Frame growth lowers operands that exceed their instruction format through dead scratch registers. Range invokes can also share an additional argument area, with entry copies preserving the body's parameter values. `Method.growLocalRegisters` reserves at least the requested number of locals; invoke lowering may require additional registers. It returns `false` without changing the body when safe lowering is unavailable. Successful growth can expand instructions and invalidate previously saved instruction indices. Register searches account for branches, exception handlers, and both words of wide values; `findFreeRegister` throws when no register is available.

`ValueRef`: `type`, `cast(type)`, `field(FieldTarget | FieldRef)`, `fieldOfType(type)`, `set(field, value)`, `assign(value)`, `call(ExtMethod | MethodTarget, args)`, `callVirtual(owner, name, proto, args)`, `callInterface(...)`, `size()`, `get(index)`, `plus`, `minus`.

| Symbol | Description |
|---|---|
| `ExtClass(name)` | A class an extension ships: `descriptor`, `target`, `static(name, params, returns)`, `method(name, params, returns)`, `field(name, type)`. |
| `ExtMethod` | `owner`, `name`, `proto`, `isStatic`, `ref`, `target`, `implement { CodeScope }`. |

### Runtime

`PatchRuntime` (receiver of `execute` and `afterDependents`): `manifest`, `resources`, `bytecode`, `files`, `options`, `log`, `explain(target)`.

| Scope | Members |
|---|---|
| `ManifestScope` | `components()`, `component(name)`, `packageName`, `versionCode`, `versionName`, `minSdkVersion`, `splitName`, `applicationClass`, `setVersionCode`, `setVersionName`, `setMinSdk`, `addPermission`, `setAttributeInt`, `setAttributeString`, `setActivityConfigChanges`, `addIntentFilter`, `addActivityAlias`, `copyIntentFilters`, `addActivity(name) { XmlElement }`, `document()`, `edit { XmlDocument }`. |
| `ResourceScope` | `components()`, `component(name)`, `owningComponent`, `id`, `exists`, `getString`, `setString`, `add`, `addString`, `addBool`, `addInteger`, `addColor`, `addDimen`, `addId`, `addRaw`, `getRaw`, `poolGet`, `poolSet`, `poolAdd`, `poolFindRefs`, `replaceEntry`. |
| `FileScope` | `components()`, `component(name)`, `list`, `read`, `source`, `signers`, `write`, `writeStored`, `delete`, `copy(bundlePath, apkPath)`, `xml(path)`, `editXml(path) { }`. |
| `BytecodeScope` | `classes`, `findClass(name)`, `classesExtending(type)`, `replaceAllStrings(old, new)`, `redirectCalls(owner, name, to)`, `redirectCalls(from: MethodRef, to)`. |
| `PatchLogger` | `info`, `warn`, `debug`. |
| `XmlDocument` | `root`, `findByTag`, `findByAttribute(name, value)`, `createElement`, `close()`; `use { }`. |
| `XmlElement` | `tag`, `parent`, `children`, `get(attr)`, `set(attr, value)`, `setInt`, `setBool`, `setResourceRef`, `removeAttribute`, `appendChild`, `insertBefore`, `remove`, `clone(deep)`. |
| `resourceRef(value)` | `@0x...` or `@ref/0x...` as a `UInt?`. |

### Bindings

| Symbol | Description |
|---|---|
| `bind(label) { BindingQuery }` | A `BindingTarget`. |
| `BindingTarget` | `sourceType`, `sourceField`, `of(value)`, `member(name, value)`. |

`BindingQuery`: `sourceType`, `fromField(label) { FieldLocator }`, `fromMethod(target)`, `fromClass(target)`, `raw { PathQuery }`, `objectValue(name) { }`, `string(name) { }`, `context(name) { }`, `intValue(name) { }`, `bind(name, target) { }`.

`FieldLocator`: `owner`, `firstObjectRead`, `firstObjectReadAnyOwner`, `nearestObjectReadBeforeString`, `rankBy(label) { RankScope }`, `requireScoreAtLeast`.

`PathQuery`: `self`, `member(name)`, `param(i)`, `field(type)`, `field(name) { FieldLocator }`, `field(target)`, `instanceField(type)`, `instanceField(types)`, `objectSlots().firstInstanceOf(type)`, `firstFieldRead`, `nextFieldRead(owner?)`, `nextInterfaceCall(returning?, returningObject)`, `callVirtual`, `callInterface`, `listGetter(name) { rankBy }`, `first`, `last`, `cast`.

### Types

| Symbol | Description |
|---|---|
| `Type` | `Void`, `Boolean`, `Byte`, `Short`, `Char`, `Int`, `Long`, `Float`, `Double`, `Object`, `String`, `CharSequence`, `List`, `ArrayList`, `Map`, `Context`, `View`, `Activity`, `Application`. |
| `descriptor(type)` | Any accepted form to a descriptor. |
| `className(descriptor)` | Descriptor to dotted name. |
| `proto(returns, vararg params)` | A method prototype string. |

## `app.reseam.patch.settings`

| Symbol | Description |
|---|---|
| `toggle(title, summary, default, key)` | Property delegate for a `ToggleSetting`. |
| `text(...)`, `folder(...)` | `TextSetting`, `FolderSetting`. |
| `choice(title, summary, default, choices, key)` | `ChoiceSetting` with `Choice(value, title)`. |
| `Setting<T>` | `key`, `title`, `summary`, `default`. |
| `section(title, vararg settings)` | A `SettingsSection`. |
| `settingsHost(appId) { }` | An internal patch installing the settings runtime: `compatibleWith`, `dependsOn`, `install { PatchRuntime }`. |
| `ReseamSettings` | The runtime `ExtClass`: `getBoolean`, `getString`. |
| `SETTINGS_SCHEMA_PATH` | `assets/reseam/settings.json`. |
| `CodeScope.whenEnabled(toggle) { }` | Branch on a toggle at runtime; returns `Otherwise`. |
| `MethodTarget.before(toggle) { }`, `.after(toggle) { }` | Gated emission. Same on `PointTarget`. |
| `MethodTarget.skipWhen(toggle)` | Return early from a void method. |
| `MethodTarget.returnTrueWhen`, `returnFalseWhen`, `returnNullWhen` | Gated constant returns. |

## `app.reseam.patch.dex`

| Symbol | Description |
|---|---|
| `Method` | Handle to a method. Reads: `info`, `classDef`, `descriptor`, `name`, `owner`, `proto`, `returnType`, `parameterTypes`, `isStatic`, `instructions`, `instructionCount`, `registersSize`, `insSize`, `outsSize`, `dexIndex`, `registerA..D(index)`, `wideLiteral`, `stringRef`, `methodRef`, `fieldRef`, `typeRef`. Searches: `indexOfFirst`, `indexOfFirstReversed`, `indexOfFirstLiteral`, `indexOfFirstLiteralReversed`, `containsLiteral`, `indexOfFirstString`, `findAllIndices`, `indexOfFirstMethodCall`, `indexOfFirstFieldAccess`, `indexOfOpcodeSequence`, `indexOfFirstInstruction { }`, `indexOfFirstInstructionReversed { }`. Mutation: `alwaysReturn*`, `setInstructions`, `replaceBody`, `insertInstruction(s)`, `addInstructions(index) { }`, `replaceInstruction`, `removeInstruction(s)`, `replaceString`, `replaceAllStrings`, `replaceLiteral`, `replaceAllLiterals`, `replaceMethodCall`, `ensureOutsSize`, `growLocalRegisters`, `findFreeRegister(s)`, `findContiguousFreeRegisters`, `setAccessFlags`, `clone`, `remove`, `addAnnotation`. |
| `DexClass` | Handle to a class: `info`, `descriptor`, `superclass`, `interfaces`, `sourceFile`, `isInterface`, `methods`, `directMethods`, `virtualMethods`, `fields`, `staticFields`, `instanceFields`, `superclassChain`, `method(name, proto?)`, `field(name)`, `setAccessFlags`, `setSuperclass`, `addInterface`, `definal`, `remove`, `addMethod`, `addField`, `removeField`, `setFieldAccessFlags`, `setStaticFieldValue`, `addAnnotation`, `addFieldAnnotation`. |
| `FieldInfo.ref` | The `FieldRef` of a field. |
| `Instruction.*` | `opcodeValue`, `opcode`, `regA`, `regB`, `regC`, `invokeRegisters`, `methodRef`, `fieldRef`, `stringValue`, `typeRef`, `literal`, `referencedRegisters`, `codeUnitSize`. |
| `MethodRef.*`, `MethodInfo.*` | `returnType`, `parameterTypes`, `descriptor`; `MethodInfo.isStatic`. |
| `parseParameterTypes(proto)`, `registerWordCount(type)`, `isReferenceType(type)` | Descriptor helpers. |
| `Opcode` | Enum of Dalvik opcodes: `value`, `isInvoke`, `isReturn`, `isMoveResult`, `rangeVariant`, `Opcode.of(value)`. |
| `AccessFlags` | Flag constants; `Int.isSet(flags)`. |
| `InstructionBuilder`, `buildInstructions { }` | See [Raw bytecode](10_dex.md#instruction-builder). |
| `lowerInvokes(insns, scratch)` | Rewrites invokes the 35c format cannot encode into range form. |
