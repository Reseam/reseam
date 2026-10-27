// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { NativeFailure } from './protocol';

/** One entry of the generated `methods.json`; types are JVM descriptors after the browser long transport. */
export interface NativeMethod { name: string; export: string; parameters: string[]; bufferLengths: Record<string, number>; returns: string }
export type EngineExports = { memory: WebAssembly.Memory } & Record<string, WebAssembly.ExportValue>;
export type NativeOutcome = { value: unknown } | { error: Uint8Array; kind: NativeFailure };
type Allocations = number[];

const ELEMENT_WIDTHS: Record<string, number> = { '[J': 8, '[I': 4, '[S': 2 };

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

function binaryArgument(exports: EngineExports, type: string, value: unknown, allocations: Allocations): number {
  if (value === null) return 0;
  if (!(value instanceof Uint8Array)) throw new Error('Expected binary native argument');
  const pointer = alloc(exports, value, ELEMENT_WIDTHS[type] ?? 1);
  allocations.push(pointer);
  return pointer;
}
function scalarArgument(type: string, value: unknown): number | bigint {
  if (type === 'J') return BigInt(value as bigint);
  if (type === 'Z') return value ? 1 : 0;
  if (typeof value !== 'number') throw new Error('Expected numeric native argument');
  return value;
}
function argument(exports: EngineExports, type: string, value: unknown, allocations: Allocations): number | bigint {
  return type.startsWith('[') || type.startsWith('L') ? binaryArgument(exports, type, value, allocations) : scalarArgument(type, value);
}
function returned(exports: EngineExports, type: string, result: number | bigint): unknown {
  if (type.startsWith('[')) {
    const pointer = Number(result);
    try { return read(exports, pointer); } finally { free(exports, pointer); }
  }
  return type === 'Z' ? !!result : type === 'V' ? null : result;
}

/** Calls a bridge export with JVM-shaped arguments; a recorded JNI exception becomes the outcome. */
export function callNative(exports: EngineExports, method: NativeMethod, args: unknown[]): NativeOutcome {
  if (args.length !== method.parameters.length) throw new Error(`Invalid arguments to ${method.name}`);
  const allocations: Allocations = [];
  fn(exports, 'reseam_bridge_reset')();
  try {
    const values = args.map((value, index) => argument(exports, method.parameters[index], value, allocations));
    const result = fn(exports, method.export)(...values);
    const kind = Number(fn(exports, 'reseam_bridge_error_kind')()) as NativeFailure | 0;
    if (kind) return { error: read(exports, Number(fn(exports, 'reseam_bridge_error')())), kind };
    return { value: returned(exports, method.returns, result) };
  } finally {
    for (const pointer of allocations) free(exports, pointer);
    fn(exports, 'reseam_bridge_reset')();
  }
}
