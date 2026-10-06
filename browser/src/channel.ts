// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

const HEADER = 24;
const CHUNK = 1 << 20;
const MAX_MESSAGE = 512 << 20;
const WAIT_MS = 120_000;
export type Packet = { kind: number; bytes: Uint8Array };

export class Channel {
  readonly control: Int32Array;
  readonly data: Uint8Array;
  constructor(readonly buffer = new SharedArrayBuffer(HEADER + CHUNK)) {
    this.control = new Int32Array(buffer, 0, 6);
    this.data = new Uint8Array(buffer, HEADER);
  }
  close(): void {
    Atomics.store(this.control, 3, 1);
    Atomics.notify(this.control, 0);
    Atomics.notify(this.control, 5);
  }
  private check(): void {
    if (Atomics.load(this.control, 3)) throw new Error('Worker channel closed');
  }
  private waitSync(value: number, index = 0): void {
    this.check();
    if (Atomics.wait(this.control, index, value, WAIT_MS) === 'timed-out') {
      this.close();
      throw new Error('Browser runtime stopped responding');
    }
    this.check();
  }
  private async waitAsync(value: number, index = 0): Promise<void> {
    this.check();
    const atomics = Atomics as typeof Atomics & {
      waitAsync?: (array: Int32Array, index: number, value: number, timeout: number) => { value: Promise<string> | string };
    };
    if (atomics.waitAsync) {
      if (await atomics.waitAsync(this.control, index, value, WAIT_MS).value === 'timed-out') {
        this.close();
        throw new Error('Browser runtime stopped responding');
      }
    } else {
      const until = performance.now() + WAIT_MS;
      while (Atomics.load(this.control, index) === value) {
        this.check();
        if (performance.now() > until) throw new Error('Browser runtime stopped responding');
        await new Promise(resolve => setTimeout(resolve, 1));
      }
    }
    this.check();
  }
  private put(kind: number, bytes: Uint8Array, offset: number): { count: number; sequence: number } {
    const count = Math.min(this.data.length, bytes.length - offset);
    this.data.set(bytes.subarray(offset, offset + count));
    Atomics.store(this.control, 1, count);
    Atomics.store(this.control, 2, offset + count === bytes.length ? 1 : 0);
    const sequence = (Atomics.add(this.control, 4, 1) + 1) | 0;
    Atomics.store(this.control, 0, kind);
    Atomics.notify(this.control, 0);
    return { count, sequence };
  }
  sendSync(kind: number, bytes: Uint8Array): void {
    this.check();
    if (bytes.length > MAX_MESSAGE) throw new Error('Bridge message exceeds 512 MiB');
    let offset = 0;
    do {
      while (Atomics.load(this.control, 0)) this.waitSync(Atomics.load(this.control, 0));
      const { count, sequence } = this.put(kind, bytes, offset);
      offset += count;
      for (;;) {
        const ack = Atomics.load(this.control, 5);
        if (ack === sequence) break;
        this.waitSync(ack, 5);
      }
    } while (offset < bytes.length);
  }
  async send(kind: number, bytes: Uint8Array): Promise<void> {
    this.check();
    if (bytes.length > MAX_MESSAGE) throw new Error('Bridge message exceeds 512 MiB');
    let offset = 0;
    do {
      while (Atomics.load(this.control, 0)) await this.waitAsync(Atomics.load(this.control, 0));
      const { count, sequence } = this.put(kind, bytes, offset);
      offset += count;
      for (;;) {
        const ack = Atomics.load(this.control, 5);
        if (ack === sequence) break;
        await this.waitAsync(ack, 5);
      }
    } while (offset < bytes.length);
  }
  private take(parts: Uint8Array[]): { kind: number; final: boolean } {
    const kind = Atomics.load(this.control, 0);
    const count = Atomics.load(this.control, 1);
    if (count < 0 || count > this.data.length) throw new Error('Invalid bridge packet length');
    parts.push(this.data.slice(0, count));
    const final = Atomics.load(this.control, 2) === 1;
    const sequence = Atomics.load(this.control, 4);
    Atomics.store(this.control, 0, 0);
    Atomics.store(this.control, 5, sequence);
    Atomics.notify(this.control, 0);
    Atomics.notify(this.control, 5);
    return { kind, final };
  }
  receiveSync(): Packet {
    this.check();
    const parts: Uint8Array[] = [];
    let kind = 0, total = 0;
    for (;;) {
      while (!Atomics.load(this.control, 0)) this.waitSync(0);
      const part = this.take(parts);
      if (kind && kind !== part.kind) throw new Error('Interleaved bridge packet');
      kind = part.kind;
      total += parts.at(-1)!.length;
      if (total > MAX_MESSAGE) throw new Error('Bridge message exceeds 512 MiB');
      if (part.final) return { kind, bytes: concat(parts, total) };
    }
  }
  async receive(): Promise<Packet> {
    this.check();
    const parts: Uint8Array[] = [];
    let kind = 0, total = 0;
    for (;;) {
      while (!Atomics.load(this.control, 0)) await this.waitAsync(0);
      const part = this.take(parts);
      if (kind && kind !== part.kind) throw new Error('Interleaved bridge packet');
      kind = part.kind;
      total += parts.at(-1)!.length;
      if (total > MAX_MESSAGE) throw new Error('Bridge message exceeds 512 MiB');
      if (part.final) return { kind, bytes: concat(parts, total) };
    }
  }
}
function concat(parts: Uint8Array[], length: number): Uint8Array {
  if (parts.length === 1) return parts[0];
  const result = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) { result.set(part, offset); offset += part.length; }
  return result;
}

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
