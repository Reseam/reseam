// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Binary buffers stay binary; only the small control envelope uses JSON.
export function encode(value: unknown): Uint8Array {
  const buffers: Uint8Array[] = [];
  const text = new TextEncoder().encode(JSON.stringify(value, (_, item: unknown) => {
    if (typeof item === 'bigint') return { $bigint: item.toString() };
    if (ArrayBuffer.isView(item)) {
      const bytes = new Uint8Array(item.buffer, item.byteOffset, item.byteLength);
      const index = buffers.push(bytes) - 1;
      return { $buffer: index };
    }
    return item;
  }));
  const length = 8 + text.length + buffers.reduce((sum, bytes) => sum + 4 + bytes.length, 0);
  const result = new Uint8Array(length);
  const view = new DataView(result.buffer);
  view.setUint32(0, text.length, true);
  view.setUint32(4, buffers.length, true);
  result.set(text, 8);
  let offset = 8 + text.length;
  for (const bytes of buffers) {
    view.setUint32(offset, bytes.length, true); offset += 4;
    result.set(bytes, offset); offset += bytes.length;
  }
  return result;
}
export function decode<T = unknown>(bytes: Uint8Array): T {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.length < 8) throw new Error('Truncated bridge envelope');
  const textLength = view.getUint32(0, true), count = view.getUint32(4, true);
  if (textLength > bytes.length - 8 || count > 1_000_000) throw new Error('Invalid bridge envelope');
  const buffers: Uint8Array[] = [];
  let offset = 8 + textLength;
  for (let i = 0; i < count; i++) {
    if (offset + 4 > bytes.length) throw new Error('Truncated bridge buffer');
    const length = view.getUint32(offset, true); offset += 4;
    if (length > bytes.length - offset) throw new Error('Truncated bridge buffer');
    buffers.push(bytes.subarray(offset, offset + length)); offset += length;
  }
  if (offset !== bytes.length) throw new Error('Trailing bridge bytes');
  return JSON.parse(new TextDecoder().decode(bytes.subarray(8, 8 + textLength)), (_, item) => {
    if (item && typeof item === 'object' && '$bigint' in item) return BigInt(item.$bigint);
    if (item && typeof item === 'object' && '$buffer' in item) {
      if (!Number.isInteger(item.$buffer) || !buffers[item.$buffer]) throw new Error('Invalid bridge buffer reference');
      return buffers[item.$buffer];
    }
    return item;
  }) as T;
}
