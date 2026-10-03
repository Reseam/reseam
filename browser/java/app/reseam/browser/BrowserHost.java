// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
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

        public void close() throws Exception { patches.clear(); references.clear(); description = null; loader.close(); }

        private Object describe(String bundle, String reference, Object patch) throws Exception {
            Object name = get(patch, "getName");
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
                options.add(map("key", get(option, "getKey"), "title", get(option, "getTitle"),
                    "description", get(option, "getDescription"), "option_type", kind,
                    "default_value", value, "valid_values", get(option, "getValidValues"), "required", get(option, "getRequired")));
            }
            return map("bundle", bundle, "id", id, "name", name == null ? id : name, "hidden", hidden,
                "description", get(patch, "getDescription"), "enabled_by_default", !hidden && (Boolean)get(patch, "getEnabled"),
                "dependencies", dependencies, "compatibility", compatibility.isEmpty() ? map("kind", "universal") : map("kind", "packages", "packages", compatibility), "options", options);
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
