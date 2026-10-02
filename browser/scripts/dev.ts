// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
export {};

// CheerpJ's loader needs a classic worker. Serve the bundled workers during
// development as well, since Vite's module-worker development transform differs.
const initial = Bun.spawn([process.execPath, 'run', 'build'], { stdout: 'inherit', stderr: 'inherit' });
if (await initial.exited) process.exit(initial.exitCode!);
const vite = 'node_modules/vite/bin/vite.js';
const children = [
  Bun.spawn([process.execPath, vite, 'build', '--watch'], { stdout: 'inherit', stderr: 'inherit' }),
  Bun.spawn([process.execPath, vite, 'preview', '--host', '127.0.0.1'], { stdout: 'inherit', stderr: 'inherit' }),
];
const stop = () => { for (const child of children) child.kill(); };
process.on('SIGINT', stop); process.on('SIGTERM', stop);
const result = await Promise.race(children.map(child => child.exited));
stop(); process.exit(result);
