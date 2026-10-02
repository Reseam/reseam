// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
export interface NativeMethod { name: string; export: string; parameters: string[]; bufferLengths: Record<string, number>; returns: string }
export type EngineExports = { memory: WebAssembly.Memory } & Record<string, WebAssembly.ExportValue>;
export function fn(exports: EngineExports, name: string): (...args: (number | bigint)[]) => number | bigint {
  const value = exports[name];
  if (typeof value !== 'function') throw new Error(`Engine is missing ${name}`);
  return value as (...args: (number | bigint)[]) => number | bigint;
}
export function alloc(exports: EngineExports, bytes: Uint8Array, width = 1): number {
  if (bytes.byteLength % width) throw new Error('Invalid bridge array width');
  const pointer = Number(fn(exports, 'reseam_buffer_alloc')(bytes.byteLength / width, width));
  if (!pointer) throw new Error('Browser engine ran out of memory');
  new Uint8Array(exports.memory.buffer, pointer + 8, bytes.length).set(bytes);
  return pointer;
}
export function read(exports: EngineExports, pointer: number): Uint8Array {
  if (!pointer) return new Uint8Array();
  const view = new DataView(exports.memory.buffer);
  if (pointer + 8 > view.byteLength) throw new Error('Invalid engine buffer');
  const length = view.getUint32(pointer, true) * view.getUint32(pointer + 4, true);
  if (length > view.byteLength - pointer - 8) throw new Error('Invalid engine buffer size');
  return new Uint8Array(exports.memory.buffer, pointer + 8, length).slice();
}
export function free(exports: EngineExports, pointer: number): void { fn(exports, 'reseam_buffer_free')(pointer); }
export function callNative(exports: EngineExports, method: NativeMethod, args: unknown[]): unknown {
  if (args.length !== method.parameters.length) throw new Error(`Invalid arguments to ${method.name}`);
  const allocations: number[] = [];
  fn(exports, 'reseam_bridge_reset')();
  try {
    const values = args.map((value, index) => {
      const type = method.parameters[index];
      if (type.startsWith('[') || type.startsWith('L')) {
        if (value === null) return 0;
        if (!(value instanceof Uint8Array)) throw new Error('Expected binary native argument');
        const width = type === '[J' ? 8 : type === '[I' ? 4 : type === '[S' ? 2 : 1;
        const pointer = alloc(exports, value, width); allocations.push(pointer); return pointer;
      }
      if (type === 'J') return BigInt(value as bigint);
      if (type === 'Z') return value ? 1 : 0;
      if (typeof value !== 'number') throw new Error('Expected numeric native argument');
      return value;
    });
    const result = fn(exports, method.export)(...values);
    const error = Number(fn(exports, 'reseam_bridge_error')());
    const kind = Number(fn(exports, 'reseam_bridge_error_kind')());
    if (kind) return { error: read(exports, error), kind };
    if (method.returns.startsWith('[')) {
      const pointer = Number(result);
      try { return { value: read(exports, pointer) }; } finally { free(exports, pointer); }
    }
    return { value: method.returns === 'Z' ? !!result : method.returns === 'V' ? null : result };
  } finally {
    for (const pointer of allocations) free(exports, pointer);
    fn(exports, 'reseam_bridge_reset')();
  }
}
