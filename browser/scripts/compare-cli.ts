// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
import { chromium } from '@playwright/test';
import { resolve } from 'node:path';
import { mkdir, rm } from 'node:fs/promises';
import { parseJson, stringifyJson } from '../src/json';

interface Case { id: string; apk: string; splits?: string[]; selection?: Record<string, unknown> }
interface Matrix { bundle: string; key: string; cert: string; trust: string; apps: Case[] }
if (!process.env.MATRIX_PATH) throw new Error('Set MATRIX_PATH to the real-app comparison configuration');
const matrix = parseJson<Matrix>(await Bun.file(process.env.MATRIX_PATH).text());
const output = resolve(process.env.OUTPUT_DIR ?? 'browser/build/comparison');
const cli = resolve(process.env.RESEAM_BIN ?? 'target/release/reseam');
const dist = resolve(import.meta.dir, '../dist');
await mkdir(output, { recursive: true });
let destination = output;
const headers = { 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp' };
const server = Bun.serve({ hostname: '127.0.0.1', port: Number(process.env.TEST_PORT ?? 18476), maxRequestBodySize: 2 ** 31, async fetch(request) {
  const path = decodeURIComponent(new URL(request.url).pathname);
  if (request.method === 'POST' && path.startsWith('/artifacts/')) {
    const target = resolve(destination, path.slice('/artifacts/'.length));
    if (!target.startsWith(destination + '/')) return new Response('Forbidden', { status: 403 });
    await mkdir(resolve(target, '..'), { recursive: true });
    const writer = Bun.file(target).writer();
    try {
      for await (const bytes of request.body!) { writer.write(bytes); await writer.flush(); }
    } finally { await writer.end(); }
    return new Response('Saved', { headers });
  }
  const target = resolve(dist, path.slice(1) || 'index.html');
  if (!target.startsWith(dist + '/')) return new Response('Forbidden', { status: 403 });
  const file = Bun.file(target);
  return new Response(await file.exists() ? file : 'Missing', { status: await file.exists() ? 200 : 404, headers });
} });
const browser = await chromium.launchPersistentContext(process.env.BROWSER_PROFILE ?? '/tmp/reseam-browser-comparison-profile', {
  executablePath: process.env.CHROMIUM_PATH, headless: true, args: ['--no-sandbox'],
});
const resultFile = Bun.file(resolve(output, 'results.json'));
const results: Record<string, unknown>[] = process.env.RESUME === '1' && await resultFile.exists() ? parseJson(await resultFile.text()) : [];
try {
  for (const app of matrix.apps) {
    if (results.some(row => row.id === app.id && !row.failed)) { console.log(`${app.id}: retaining successful comparison`); continue; }
    if (!/^[a-z0-9-]+$/.test(app.id)) throw new Error('Invalid comparison case id');
    const directory = resolve(output, app.id);
    await rm(directory, { recursive: true, force: true });
    const nativeOutput = resolve(directory, 'cli');
    destination = resolve(directory, 'browser');
    await mkdir(nativeOutput, { recursive: true }); await mkdir(destination, { recursive: true });
    const selection = { preset: 'recommended', enable: [], disable: [], options: {}, ...app.selection };
    const request = {
      apk_path: resolve(app.apk), split_paths: (app.splits ?? []).map(path => resolve(path)),
      bundle_paths: [resolve(matrix.bundle)], trust: { keys: [matrix.trust] }, selection,
      output: { kind: 'split_dir', path: nativeOutput }, signing: { key: resolve(matrix.key), cert: resolve(matrix.cert) },
    };
    console.log(`${app.id}: patching with CLI`);
    const started = performance.now();
    const native = Bun.spawn([cli, 'perf-worker'], { stdin: 'pipe', stdout: 'pipe', stderr: Bun.file(resolve(directory, 'cli.log')) });
    native.stdin.write(stringifyJson({ request, options: [] })); native.stdin.end();
    const nativeText = await new Response(native.stdout).text();
    const nativeStatus = await native.exited;
    const nativeResult = nativeStatus === 0 ? parseJson<any>(nativeText) : { status: 'failure', error: `CLI exited ${nativeStatus}` };
    await Bun.write(resolve(directory, 'cli-result.json'), stringifyJson(nativeResult));
    const row: Record<string, unknown> = { id: app.id, apk: resolve(app.apk), cli: nativeResult, cliWallMs: Math.round(performance.now() - started) };
    console.log(`${app.id}: patching in Chromium`);
    const page = await browser.newPage();
    const log: string[] = [];
    page.on('console', message => { log.push(message.text()); if (message.text().startsWith('event')) console.log(`${app.id}: ${message.text().slice(0, 250)}`); });
    page.on('pageerror', error => log.push(String(error)));
    try {
      await page.goto(server.url.href);
      await page.locator('#apk').setInputFiles(resolve(app.apk));
      await page.locator('#splits').setInputFiles((app.splits ?? []).map(path => resolve(path)));
      await page.locator('#bundles').setInputFiles(resolve(matrix.bundle));
      await page.locator('#key').setInputFiles(resolve(matrix.key)); await page.locator('#cert').setInputFiles(resolve(matrix.cert));
      const result = await page.evaluate(async ({ selection, trust, trace }) => {
        const selected = (id: string) => [...((document.getElementById(id) as HTMLInputElement).files ?? [])];
        const apk = selected('apk')[0], bundles = selected('bundles'), splits = selected('splits');
        const files = [...[apk, ...splits, ...bundles].map(file => ({ name: file.name, file })),
          { name: 'reseam.pk8', file: selected('key')[0], directory: 'identity' },
          { name: 'reseam.der', file: selected('cert')[0], directory: 'identity' }];
        const started = performance.now();
        const Session = (window as any).BrowserSession;
        if (!Session) throw new Error('Build the comparison application with bun run build:test');
        const session = await Session.open(files, { traceNative: trace, onEvent: (event: unknown) => console.log('event', JSON.stringify(event)) });
        try {
          const apkPath = '/input/' + apk.name, splitPaths = splits.map(file => '/input/' + file.name), bundlePaths = bundles.map(file => '/input/' + file.name);
          const inspection = await session.request('inspect', { apk_path: apkPath, split_paths: splitPaths, bundle_paths: bundlePaths });
          const patchStarted = performance.now();
          const outcome = await session.request('patch', { apk_path: apkPath, split_paths: splitPaths, bundle_paths: bundlePaths,
            trust: { keys: [trust] }, selection, output: { kind: 'split_dir', path: '/output/patched' },
            signing: { key: '/identity/reseam.pk8', cert: '/identity/reseam.der' } });
          const patchWallMs = Math.round(performance.now() - patchStarted);
          const artifacts = await session.artifacts();
          for (const artifact of artifacts) {
            if (!artifact.name.endsWith('.apk')) continue;
            const response = await fetch('/artifacts/' + artifact.name.replace(/^patched\//, ''), { method: 'POST', body: artifact.file });
            if (!response.ok) throw new Error(`Could not export browser APK (${response.status})`);
          }
          return { inspection, outcome, patchWallMs, totalWallMs: Math.round(performance.now() - started), wasmMemoryBytes: session.wasmMemoryBytes };
        } finally { await session.dispose(); }
      }, { selection, trust: matrix.trust, trace: process.env.TRACE_NATIVE === '1' });
      row.browser = result;
      await Bun.write(resolve(directory, 'browser-result.json'), stringifyJson(result));
      const comparison = Bun.spawn(['python3', resolve(import.meta.dir, 'compare-apks.py'), nativeOutput, destination], { stdout: 'pipe', stderr: 'pipe' });
      const compared = await new Response(comparison.stdout).text();
      const comparisonErrors = await new Response(comparison.stderr).text();
      row.comparison = compared ? parseJson(compared) : { error: comparisonErrors };
      if (await comparison.exited) row.failed = true;
      const summary = row.comparison as { apks?: { bytes_outside_signing_block_identical: boolean; different_entries: string[]; signatures: { valid: boolean }[] }[] };
      console.log(`${app.id}: compared ${summary.apks?.length ?? 0} APKs; matching bytes outside signing block: ${summary.apks?.every(apk => apk.bytes_outside_signing_block_identical)}; all signatures valid: ${summary.apks?.every(apk => apk.signatures.every(signature => signature.valid))}`);
    } catch (error) { row.browserError = String(error); row.failed = true; console.error(`${app.id}: ${String(error)}`); }
    finally { await Bun.write(resolve(directory, 'browser.log'), log.join('\n')); await page.close(); }
    if (nativeResult.status !== 'success') row.failed = true;
    const existing = results.findIndex(item => item.id === app.id);
    if (existing >= 0) results[existing] = row; else results.push(row);
    await Bun.write(resolve(output, 'results.json'), stringifyJson(results));
  }
} finally { await browser.close(); server.stop(); }
if (results.some(row => row.failed)) process.exitCode = 1;
