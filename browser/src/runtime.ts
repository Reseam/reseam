// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { runtime } from './runtime.generated';
export { runtime };
export type RuntimeAsset = keyof typeof runtime.assets;
export async function loadRuntime(base: string, asset: RuntimeAsset): Promise<Uint8Array> {
  const response = await fetch(new URL(asset, base));
  if (!response.ok) throw new Error(`Could not load patch runtime (${response.status})`);
  const expected = runtime.assets[asset];
  const declared = response.headers.get('Content-Length');
  if (!response.headers.get('Content-Encoding') && declared !== null && Number(declared) !== expected.size) throw new Error('Patch runtime asset has an unexpected size');
  if (!response.body) throw new Error('Patch runtime asset has no response body');
  const bytes = new Uint8Array(expected.size);
  const reader = response.body.getReader();
  let offset = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value.length > bytes.length - offset) throw new Error('Patch runtime asset exceeds its expected size');
      bytes.set(value, offset); offset += value.length;
    }
  } catch (error) { await reader.cancel().catch(() => {}); throw error; }
  finally { reader.releaseLock(); }
  if (offset !== expected.size) throw new Error('Patch runtime asset is truncated');
  const hash = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(byte => byte.toString(16).padStart(2, '0')).join('');
  if (bytes.length !== expected.size || hash !== expected.sha256) throw new Error('Patch runtime files do not match this application version. Reload after the deployment finishes.');
  return bytes;
}
