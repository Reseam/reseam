// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

const bundles = new Map();

// The settings screen offers a restart after changes, so one read per process is enough.
export function isEnabled(bundle, key, defaultValue) {
    if (!bundles.has(bundle)) bundles.set(bundle, new Map());
    const values = bundles.get(bundle);
    if (!values.has(key)) {
        values.set(key, globalThis.nativeModuleProxy.ReseamSettings.isEnabled(bundle, key, defaultValue));
    }
    return values.get(key);
}

// A copy of `object` whose property at the dotted `path` is `value`; each object along it is copied.
export function withProperty(object, path, value) {
    const [name, ...rest] = path.split(".");
    return { ...object, [name]: rest.length === 0 ? value : withProperty(object[name], rest.join("."), value) };
}
