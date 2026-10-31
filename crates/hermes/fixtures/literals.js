// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

// An extension module whose literals the compiler stores in shared byte runs: some references
// start inside another literal's run.
(function () {
    'use strict';
    return {
        imports: [],
        link: function () {
            var exports = {};
            const LEVELS = ["None", "Low", "Medium", "High", "Highest"];
            function formatDate(date) {
                return date.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
            }
            exports.rows = function (record, count) {
                return [
                    ["Owner", record.owner],
                    ["Created", formatDate(record.created)],
                    ["Joined", record.joined == null ? null : formatDate(record.joined)],
                    ["Members", count == null ? null : count.toLocaleString()],
                    ["Roles", String(record.roles)],
                    ["Level", LEVELS[record.level] ?? null],
                    ["ID", record.id],
                ].filter(([, value]) => value != null);
            };
            exports.render = function (h, design, info) {
                return h(design.Sheet, null,
                    h(design.Header, { title: info.name }),
                    h(design.Group, null, ...info.rows.map(([label, value]) =>
                        h(design.Row, { key: label, label, trailing: h(design.Text, { text: value }) }))));
            };
            return exports;
        },
    };
})();
