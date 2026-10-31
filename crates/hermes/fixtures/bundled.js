// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

// The app runtime extension as the Gradle plugin bundles it: esbuild's IIFE output, loaded on
// first use inside the `{ imports, link }` declaration the engine links.
(function () {
    "use strict";
    return {
        imports: [],
        link: function (modules) {
            var namespaces = {};
            var extension;
            // An app module may still be initializing when the extension loads, so each import
            // reads it through Metro on first use, as Metro's inline requires do. The engine names
            // the module's exports, which esbuild's interop copies as getters.
            function require(specifier) {
                var module = modules[specifier];
                var exports;
                function read() {
                    if (exports === undefined) exports = globalThis.__r(module.id);
                    return exports;
                }
                return namespaces[specifier] || (namespaces[specifier] = new Proxy({}, {
                    get: function (_, key) { return key === "__esModule" ? module.esModule : read()[key]; },
                    has: function (_, key) { return key in read(); },
                    ownKeys: function () { return module.keys.slice(); },
                    getOwnPropertyDescriptor: function (_, key) {
                        return module.keys.indexOf(key) === -1
                            ? undefined
                            : { configurable: true, enumerable: true, writable: false, value: undefined };
                    },
                    getPrototypeOf: function () { return Object.prototype; },
                }));
            }
            // App modules exist only once the app's own global code has run, so the extension
            // loads on its first call.
            function load() {
                if (extension === undefined) {
                    var reseamExtension = (() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, { get: all[name], enumerable: true });
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      for (let key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
    }
    return to;
  };
  var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

  // src/main/js/index.js
  var index_exports = {};
  __export(index_exports, {
    isEnabled: () => isEnabled,
    withProperty: () => withProperty
  });
  var bundles = /* @__PURE__ */ new Map();
  function isEnabled(bundle, key, defaultValue) {
    if (!bundles.has(bundle)) bundles.set(bundle, /* @__PURE__ */ new Map());
    const values = bundles.get(bundle);
    if (!values.has(key)) {
      values.set(key, globalThis.nativeModuleProxy.ReseamSettings.isEnabled(bundle, key, defaultValue));
    }
    return values.get(key);
  }
  function withProperty(object, path, value) {
    const [name, ...rest] = path.split(".");
    return { ...object, [name]: rest.length === 0 ? value : withProperty(object[name], rest.join("."), value) };
  }
  return __toCommonJS(index_exports);
})();

                    extension = reseamExtension;
                }
                return extension;
            }
            return {
                "isEnabled": function () { return load()["isEnabled"].apply(this, arguments); },
                "withProperty": function () { return load()["withProperty"].apply(this, arguments); },
            };
        },
    };
})();
