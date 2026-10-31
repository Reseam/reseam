// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

// An app bundle's global code as Metro emits it: each module factory is registered with `__d`.
__d(function (global, require, importDefault, importAll, module, exports, dependencyMap) {
    exports.greet = function greet(name) {
        return "hello " + name;
    };
}, 7, []);
__d(function (global, require, importDefault, importAll, module, exports, dependencyMap) {
    exports.store = {
        getCount: function getCount() {
            return 300;
        },
    };
}, 300, [7]);
__d(function (global, require, importDefault, importAll, module, exports, dependencyMap) {
    Object.defineProperty(exports, "__esModule", { value: true });
    exports.default = { getTheme: function getTheme() { return "dark"; } };
    exports.ThemeTypes = { DARK: "dark" };
}, 301, []);
__d(function (global, require, importDefault, importAll, module, exports, dependencyMap) {
    module.exports = {
        get View() {
            return function View() {};
        },
        get Text() {
            return function Text() {};
        },
    };
}, 302, []);
