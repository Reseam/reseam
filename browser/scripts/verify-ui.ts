// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
// Exercise the actual basic UI with an app from the real-app comparison matrix.
import { chromium, expect } from '@playwright/test';
import { resolve } from 'node:path';
import { mkdir } from 'node:fs/promises';
import { parseJson, stringifyJson } from '../src/json';

if (!process.env.MATRIX_PATH) throw new Error('Set MATRIX_PATH to the real-app comparison matrix');
const matrix = parseJson<any>(await Bun.file(process.env.MATRIX_PATH).text());
const app = matrix.apps.find((entry: any) => entry.id === process.env.APP_ID) ?? matrix.apps[0];
const dist = resolve(import.meta.dir, '../dist');
const output = resolve(process.env.OUTPUT_DIR ?? 'browser/build/ui-verification');
await mkdir(output, { recursive: true });
const server = Bun.serve({ hostname: '127.0.0.1', port: Number(process.env.TEST_PORT ?? 18477), async fetch(request) {
  const path = resolve(dist, decodeURIComponent(new URL(request.url).pathname).slice(1) || 'index.html');
  if (!path.startsWith(dist + '/')) return new Response('Forbidden', { status: 403 });
  const file = Bun.file(path);
  if (!await file.exists()) return new Response('Missing', { status: 404 });
  const headers: Record<string, string> = { 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp' };
  if (process.env.GZIP_ASSETS === '1' && path.includes('/runtime/')) {
    const compressed = Bun.gzipSync(await file.bytes());
    headers['Content-Encoding'] = 'gzip'; headers['Content-Length'] = String(compressed.length);
    headers['Content-Type'] = file.type;
    return new Response(compressed, { headers });
  }
  return new Response(file, { headers });
} });
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH, headless: true, args: ['--no-sandbox'] });
const context = await browser.newContext({ acceptDownloads: true });
const page = await context.newPage();
const log: string[] = [];
page.on('console', message => log.push(message.text()));
page.on('pageerror', error => log.push(String(error)));
try {
  await page.goto(server.url.href);
  await page.locator('#apk').setInputFiles(resolve(app.apk));
  await page.locator('#splits').setInputFiles((app.splits ?? []).map((path: string) => resolve(path)));
  await page.locator('#bundles').setInputFiles(resolve(matrix.bundle));
  // Termination during initialization must leave the page usable for a new run.
  await page.locator('#inspect').click();
  await page.locator('#cancel').click();
  await expect(page.locator('#inspect')).toBeEnabled({ timeout: 30_000 });
  await page.locator('#inspect').click();
  await expect(page.locator('#configuration')).toBeVisible({ timeout: 180_000 });
  await expect(page.locator('#patch')).toBeDisabled();
  await page.locator('#trust input').check();
  await page.locator('#key').setInputFiles(resolve(matrix.key));
  await page.locator('#cert').setInputFiles(resolve(matrix.cert));
  await page.locator('#patch').click();
  await expect(page.locator('#status')).toHaveText('Patch complete', { timeout: 600_000 });
  const files: string[] = [];
  const links = await page.locator('#downloads a').all();
  if (links.length < 3) throw new Error('APK and identity download links are missing');
  // The full comparison runner exports every split. Exercise real browser
  // downloads here without triggering Chromium's burst-download throttling.
  for (const link of [links[0], ...links.slice(-2)]) {
    const download = page.waitForEvent('download');
    await link.click();
    const artifact = await download;
    const name = artifact.suggestedFilename();
    await artifact.saveAs(resolve(output, name)); files.push(name);
  }
  const identityMatches = await Bun.file(resolve(output, 'reseam.pk8')).bytes().then(async key =>
    Buffer.from(key).equals(Buffer.from(await Bun.file(matrix.key).bytes())) &&
    Buffer.from(await Bun.file(resolve(output, 'reseam.der')).bytes()).equals(Buffer.from(await Bun.file(matrix.cert).bytes())));
  if (!identityMatches) throw new Error('UI used a different signing identity');
  await page.reload();
  await page.locator('#apk').setInputFiles(resolve(app.apk));
  await page.locator('#splits').setInputFiles((app.splits ?? []).map((path: string) => resolve(path)));
  await page.locator('#bundles').setInputFiles(resolve(matrix.bundle));
  await page.locator('#inspect').click();
  await expect(page.locator('#configuration')).toBeVisible({ timeout: 180_000 });
  await expect(page.locator('#trust input')).toBeChecked();
  const persisted = await page.evaluate(async () => {
    const request = indexedDB.open('reseam-browser');
    const db = await new Promise<IDBDatabase>((resolve, reject) => { request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error); });
    try {
      const lookup = db.transaction('preferences', 'readonly').objectStore('preferences').get('identity');
      const identity = await new Promise<any>((resolve, reject) => { lookup.onsuccess = () => resolve(lookup.result); lookup.onerror = () => reject(lookup.error); });
      return identity ? { key: [...new Uint8Array(identity.key)], cert: [...new Uint8Array(identity.cert)] } : undefined;
    } finally { db.close(); }
  });
  if (!persisted || !Buffer.from(persisted.key).equals(Buffer.from(await Bun.file(matrix.key).bytes())) || !Buffer.from(persisted.cert).equals(Buffer.from(await Bun.file(matrix.cert).bytes()))) throw new Error('Signing identity did not persist after reload');
  await Bun.write(resolve(output, 'result.json'), stringifyJson({ app: app.id, gzipRuntimeAssets: process.env.GZIP_ASSETS === '1', cancellation: true, signerApproval: true, importedIdentity: identityMatches, persistedIdentity: true, rememberedApproval: true, downloadLinks: links.length, downloads: files }));
  console.log(`${app.id}: UI patching, cancellation, signing identity, downloads and persistence passed`);
} finally {
  await Bun.write(resolve(output, 'browser.log'), log.join('\n'));
  await browser.close(); server.stop();
}
