// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later
package app.reseam.browser;

import com.google.gson.Gson;
import java.io.File;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/** Hosts unmodified patch JARs under one isolated class loader per bundle. */
public final class BrowserHost {
    private BrowserHost() {}

    public static byte[] bytes(ByteBuffer buffer, int length) {
        if (length < 0 || length > buffer.capacity()) throw new IllegalArgumentException("Invalid native buffer length");
        byte[] bytes = new byte[length];
        ByteBuffer view = buffer.duplicate();
        view.clear();
        view.get(bytes);
        return bytes;
    }

    public static Bundle load(String runtime, String[] jars, String[] classes, String[] owners,
                              String[] members, String[] kinds, String[] ids, String bundle) throws Exception {
        return new Bundle(runtime, jars, classes, owners, members, kinds, ids, bundle);
    }

    public static final class Bundle implements AutoCloseable {
        private final URLClassLoader loader;
        private final LinkedHashMap<String, Object> patches = new LinkedHashMap<>();
        private final IdentityHashMap<Object, String> references = new IdentityHashMap<>();
        private final Class<?> external;
        private final Class<?> runtime;
        private final Object run;
        private final Method beginInvocation;
        private final Method endInvocation;
        private final StringTables tables;
        private String description;

        Bundle(String runtimePath, String[] jars, String[] classes, String[] owners,
               String[] members, String[] kinds, String[] ids, String bundle) throws Exception {
            if (classes.length != owners.length || classes.length != members.length ||
                classes.length != kinds.length || classes.length != ids.length) {
                throw new IllegalArgumentException("Mismatched patch index columns");
            }
            URL[] urls = new URL[jars.length + 1];
            urls[0] = new File(runtimePath).toURI().toURL();
            for (int i = 0; i < jars.length; i++) urls[i + 1] = new File(jars[i]).toURI().toURL();
            loader = new URLClassLoader(urls, ClassLoader.getPlatformClassLoader());
            try {
                Class<?> patchClass = loader.loadClass("app.reseam.patch.ReseamPatch");
                external = loader.loadClass("app.reseam.patch.ExternalPatch");
                runtime = loader.loadClass("app.reseam.patch.PatchRuntime");
                tables = new StringTables(loader.loadClass("app.reseam.patch.settings.StringTableFiles"));
                Class<?> nativeClass = loader.loadClass("app.reseam.patch.native.Native");
                beginInvocation = nativeClass.getMethod("beginBrowserInvocation", long[].class);
                endInvocation = nativeClass.getMethod("endBrowserInvocation");
                beginInvocation.setAccessible(true);
                endInvocation.setAccessible(true);
                Class<?> runClass = loader.loadClass("app.reseam.patch.PatchRun");
                run = runClass.getConstructor().newInstance();
                for (int i = 0; i < classes.length; i++) {
                    Class<?> clazz = loader.loadClass(classes[i]);
                    Object value;
                    if (kinds[i].equals("field")) {
                        java.lang.reflect.Field field = clazz.getField(members[i]);
                        if (!field.getDeclaringClass().getName().equals(owners[i])) throw new IllegalArgumentException("Patch owner differs from index");
                        value = field.get(null);
                    } else if (kinds[i].equals("method")) {
                        Method method = clazz.getMethod(members[i]);
                        if (!method.getDeclaringClass().getName().equals(owners[i])) throw new IllegalArgumentException("Patch owner differs from index");
                        value = method.invoke(null);
                    } else throw new IllegalArgumentException("Unknown patch declaration kind");
                    if (!patchClass.isInstance(value)) throw new IllegalArgumentException("Patch declaration is not a ReseamPatch");
                    if (external.isInstance(value)) continue;
                    String reference = bundle + "/" + ids[i];
                    String previous = references.get(value);
                    if (previous == null) references.put(value, reference);
                    else if (reference.compareTo(previous) < 0) references.put(value, reference);
                    patches.putIfAbsent(reference, value);
                }
                LinkedHashMap<String, Object> canonical = new LinkedHashMap<>();
                for (Object value : patches.values()) canonical.put(references.get(value), value);
                patches.clear();
                patches.putAll(canonical);
                List<Object> specs = new ArrayList<>();
                for (Map.Entry<String, Object> entry : patches.entrySet()) specs.add(map("spec", describe(bundle, entry.getKey(), entry.getValue()), "finalizes", get(entry.getValue(), "getFinalizes")));
                description = new Gson().toJson(specs);
            } catch (Exception error) {
                loader.close();
                throw error;
            }
        }

        public String describe() { return description; }

        public void invoke(String reference, String phase, long[] revision) throws Exception {
            Object patch = patches.get(reference);
            if (patch == null) throw new IllegalArgumentException("Unknown patch " + reference);
            String method = phase.equals("execute") ? "invokeExecute" : phase.equals("finalize") ? "invokeAfterDependents" : null;
            if (method == null) throw new IllegalArgumentException("Unknown patch phase");
            beginInvocation.invoke(null, (Object)revision);
            try {
                Object context = runtime.getConstructor(run.getClass()).newInstance(run);
                runtime.getMethod(method, loader.loadClass("app.reseam.patch.ReseamPatch")).invoke(context, patch);
            }
            catch (InvocationTargetException error) {
                Throwable cause = error.getCause();
                if (cause instanceof Exception) throw (Exception)cause;
                throw error;
            } finally { endInvocation.invoke(null); }
        }

        public void savepoint(String action, int savepoint) throws Exception {
            String method = action.equals("open") ? "openSavepoint" : action.equals("release") ? "releaseSavepoint" : action.equals("rollback") ? "rollbackTo" : null;
            if (method == null) throw new IllegalArgumentException("Unknown savepoint action");
            run.getClass().getMethod(method, int.class).invoke(run, savepoint);
        }

                public void close() throws Exception { patches.clear(); references.clear(); description = null; loader.close(); }

        private Object describe(String bundle, String reference, Object patch) throws Exception {
            Text name = tables.text(get(patch, "getName"));
            Text patchDescription = tables.text(get(patch, "getDescription"));
            TreeMap<String, Map<String, Object>> translations = new TreeMap<>();
            if (name != null) name.translations().forEach((locale, text) -> translation(translations, locale).put("name", text));
            if (patchDescription != null) patchDescription.translations().forEach((locale, text) -> translation(translations, locale).put("description", text));
            String id = reference.substring(bundle.length() + 1);
            boolean hidden = name == null || (Boolean)get(patch, "getHidden");
            List<String> dependencies = new ArrayList<>();
            for (Object dependency : list(get(patch, "getDependencies"))) {
                if (external.isInstance(dependency)) dependencies.add(get(dependency, "getBundle") + "/" + get(dependency, "getId"));
                else {
                    String target = references.get(dependency);
                    if (target == null) throw new IllegalArgumentException("Undeclared dependency of " + reference);
                    dependencies.add(target);
                }
            }
            List<Object> compatibility = new ArrayList<>();
            for (Object pkg : list(get(patch, "getCompatibleWith"))) compatibility.add(map("package", get(pkg, "getName"), "versions", get(pkg, "getVersions")));
            List<Object> options = new ArrayList<>();
            for (Object option : list(get(patch, "getOptions"))) {
                String kind = get(option, "getKind").toString().toLowerCase(java.util.Locale.ROOT);
                Object value = get(option, "getDefault");
                if (value != null) value = map("type", kind, "value", value);
                String key = (String)get(option, "getKey");
                Text title = tables.text(get(option, "getTitle"));
                Text optionDescription = tables.text(get(option, "getDescription"));
                title.translations().forEach((locale, text) -> optionTranslation(translation(translations, locale), key).put("title", text));
                if (optionDescription != null) optionDescription.translations().forEach((locale, text) -> optionTranslation(translation(translations, locale), key).put("description", text));
                options.add(map("key", key, "title", title.source(),
                    "description", optionDescription == null ? "" : optionDescription.source(), "option_type", kind,
                    "default_value", value, "valid_values", get(option, "getValidValues"), "required", get(option, "getRequired")));
            }
            return map("bundle", bundle, "id", id, "name", name == null ? id : name.source(), "hidden", hidden,
                "description", patchDescription == null ? "" : patchDescription.source(), "enabled_by_default", !hidden && (Boolean)get(patch, "getEnabled"),
                "dependencies", dependencies, "compatibility", compatibility.isEmpty() ? map("kind", "universal") : map("kind", "packages", "packages", compatibility), "options", options,
                "rewrites_output", get(patch, "getRewritesOutput"), "translations", new ArrayList<>(translations.values()));
        }

        private static Map<String, Object> translation(TreeMap<String, Map<String, Object>> translations, String locale) {
            return translations.computeIfAbsent(locale, tag -> map("locale", tag, "options", new ArrayList<Map<String, Object>>()));
        }

        @SuppressWarnings("unchecked")
        private static Map<String, Object> optionTranslation(Map<String, Object> translation, String key) {
            List<Map<String, Object>> options = (List<Map<String, Object>>)translation.get("options");
            for (Map<String, Object> option : options) if (option.get("key").equals(key)) return option;
            Map<String, Object> option = map("key", key);
            options.add(option);
            return option;
        }
    }

    /** A string's source text, and its text in each locale that translates it. */
    private record Text(String source, TreeMap<String, String> translations) {}

    /** The string tables a bundle's patches name, read once each through the patch API. */
    private static final class StringTables {
        private final Method locales;
        private final Method json;
        private final IdentityHashMap<Object, LinkedHashMap<String, Map<String, Object>>> tables = new IdentityHashMap<>();

        StringTables(Class<?> files) throws Exception {
            Class<?> table = files.getClassLoader().loadClass("app.reseam.patch.settings.StringTable");
            locales = files.getMethod("locales", table);
            json = files.getMethod("json", table, String.class);
        }

        @SuppressWarnings("unchecked")
        Text text(Object reference) throws Exception {
            if (reference == null) return null;
            Object table = get(reference, "getTable");
            String name = (String)get(reference, "getName");
            LinkedHashMap<String, Map<String, Object>> byLocale = tables.get(table);
            if (byLocale == null) {
                byLocale = new LinkedHashMap<>();
                for (Object locale : (List<?>)locales.invoke(null, table)) {
                    String json = new String((byte[])this.json.invoke(null, table, locale), java.nio.charset.StandardCharsets.UTF_8);
                    byLocale.put((String)locale, new Gson().fromJson(json, Map.class));
                }
                tables.put(table, byLocale);
            }
            String source = null;
            TreeMap<String, String> translations = new TreeMap<>();
            for (Map.Entry<String, Map<String, Object>> entry : byLocale.entrySet()) {
                Object value = entry.getValue().get(name);
                if (value == null) continue;
                if (!(value instanceof String text)) throw new IllegalArgumentException("String '" + name + "' of " + get(table, "getId") + " is a plural; patch text must be plain");
                if (entry.getKey().equals("default")) source = text;
                else translations.put(entry.getKey(), text);
            }
            if (source == null) throw new IllegalArgumentException(get(table, "getId") + " has no source text for '" + name + "'");
            return new Text(source, translations);
        }
    }

    private static Object get(Object object, String method) throws Exception { return object.getClass().getMethod(method).invoke(object); }
    private static List<?> list(Object value) { return (List<?>)value; }
    private static Map<String, Object> map(Object... pairs) {
        Map<String, Object> result = new LinkedHashMap<>();
        for (int i = 0; i < pairs.length; i += 2) result.put((String)pairs[i], pairs[i + 1]);
        return result;
    }
}
