// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Endpoint, PacketKind } from './protocol';

const HEADER = 24;
const CHUNK = 1 << 20;
const MAX_MESSAGE = 512 << 20;
const WAIT_MS = 120_000;
export type Packet = { kind: PacketKind; bytes: Uint8Array };
/** A control word a transfer waits on until it changes from `value`. */
type Wait = { value: number; index: number };
/**
 * A port pair sharing one channel. The client posts requests on its port and reads replies from
 * the channel; the server receives requests on its port and writes replies to the channel.
 */
export interface Link { channel: Channel; client: Endpoint; server: Endpoint }

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
  get closed(): boolean { return Atomics.load(this.control, 3) === 1; }
  private check(): void {
    if (this.closed) throw new Error('Worker channel closed');
  }
  private waitSync(value: number, index: number): void {
    this.check();
    if (Atomics.wait(this.control, index, value, WAIT_MS) === 'timed-out') {
      this.close();
      throw new Error('Browser runtime stopped responding');
    }
    this.check();
  }
  private async waitAsync(value: number, index: number): Promise<void> {
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
  private put(kind: PacketKind, bytes: Uint8Array, offset: number): { count: number; sequence: number } {
    const count = Math.min(this.data.length, bytes.length - offset);
    this.data.set(bytes.subarray(offset, offset + count));
    Atomics.store(this.control, 1, count);
    Atomics.store(this.control, 2, offset + count === bytes.length ? 1 : 0);
    const sequence = (Atomics.add(this.control, 4, 1) + 1) | 0;
    Atomics.store(this.control, 0, kind);
    Atomics.notify(this.control, 0);
    return { count, sequence };
  }
  sendSync(kind: PacketKind, bytes: Uint8Array): void {
    this.runSync(this.sending(kind, bytes));
  }
  async send(kind: PacketKind, bytes: Uint8Array): Promise<void> {
    await this.run(this.sending(kind, bytes));
  }
  receiveSync(): Packet {
    return this.runSync(this.receiving());
  }
  receive(): Promise<Packet> {
    return this.run(this.receiving());
  }
  private runSync<T>(steps: Generator<Wait, T>): T {
    for (let step = steps.next(); ; step = steps.next()) {
      if (step.done) return step.value;
      this.waitSync(step.value.value, step.value.index);
    }
  }
  private async run<T>(steps: Generator<Wait, T>): Promise<T> {
    for (let step = steps.next(); ; step = steps.next()) {
      if (step.done) return step.value;
      await this.waitAsync(step.value.value, step.value.index);
    }
  }
  private *sending(kind: PacketKind, bytes: Uint8Array): Generator<Wait, void> {
    this.check();
    if (bytes.length > MAX_MESSAGE) throw new Error('Bridge message exceeds 512 MiB');
    let offset = 0;
    do {
      for (let pending; (pending = Atomics.load(this.control, 0));) yield { value: pending, index: 0 };
      const { count, sequence } = this.put(kind, bytes, offset);
      offset += count;
      for (let ack; (ack = Atomics.load(this.control, 5)) !== sequence;) yield { value: ack, index: 5 };
    } while (offset < bytes.length);
  }
  private *receiving(): Generator<Wait, Packet> {
    this.check();
    const parts: Uint8Array[] = [];
    let kind: PacketKind | 0 = 0, total = 0;
    for (;;) {
      while (!Atomics.load(this.control, 0)) yield { value: 0, index: 0 };
      const part = this.take(parts);
      if (kind && kind !== part.kind) throw new Error('Interleaved bridge packet');
      kind = part.kind;
      total += parts.at(-1)!.length;
      if (total > MAX_MESSAGE) throw new Error('Bridge message exceeds 512 MiB');
      if (part.final) return { kind, bytes: concat(parts, total) };
    }
  }
  private take(parts: Uint8Array[]): { kind: PacketKind; final: boolean } {
    const kind: PacketKind = Atomics.load(this.control, 0);
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
}
export function link(): Link {
  const channel = new Channel();
  const { port1, port2 } = new MessageChannel();
  return { channel, client: { port: port1, buffer: channel.buffer }, server: { port: port2, buffer: channel.buffer } };
}
function concat(parts: Uint8Array[], length: number): Uint8Array {
  if (parts.length === 1) return parts[0];
  const result = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) { result.set(part, offset); offset += part.length; }
  return result;
}
