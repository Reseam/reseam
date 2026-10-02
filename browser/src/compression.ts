// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
import type { WASI } from '@bjorn3/browser_wasi_shim';
import { Channel, decode } from './channel';
import { DiskDescriptor, DiskFile } from './storage';
import type { EngineExports } from './native';

export interface CompressionConnection { port: MessagePort; buffer: SharedArrayBuffer }

export class CompressionPool {
  private readonly slots: { port: MessagePort; channel: Channel; output?: DiskFile; name?: string }[];
  memoryBytes = 0;
  constructor(connections: CompressionConnection[], private readonly wasi: WASI, private readonly exports: () => EngineExports) {
    this.slots = connections.map(connection => ({ port: connection.port, channel: new Channel(connection.buffer) }));
  }
  readonly imports = {
    workers: (): number => this.slots.length,
    submit: (inputFd: number, outputFd: number, pointer: number, length: number, level: bigint): number => {
      const index = this.slots.findIndex(slot => !slot.output);
      if (index < 0) throw new Error('Compression queue exceeded its capacity');
      const input = this.file(inputFd), output = this.file(outputFd);
      const name = new TextDecoder().decode(new Uint8Array(this.exports().memory.buffer, pointer, length));
      const slot = this.slots[index];
      slot.output = output; slot.name = name;
      slot.port.postMessage({ input: input.id, output: output.id, length: input.size, name, level });
      return index;
    },
    finish: (ticket: number): number => {
      const slot = this.slots[ticket];
      if (!slot?.output) throw new Error('Unknown compression job');
      const packet = slot.channel.receiveSync();
      if (packet.kind !== 5) throw new Error('Unexpected compression reply');
      const response = decode<{ size: bigint; memoryBytes: number; error?: string }>(packet.bytes);
      if (response.error) throw new Error(`Compressing ${slot.name}: ${response.error}`);
      slot.output.writtenExternally(response.size);
      this.memoryBytes = Math.max(this.memoryBytes, response.memoryBytes * this.slots.length);
      slot.output = undefined; slot.name = undefined;
      return 0;
    },
  };
  private file(fd: number): DiskFile {
    const descriptor = this.wasi.fds[fd];
    if (!(descriptor instanceof DiskDescriptor) || !(descriptor.file instanceof DiskFile)) throw new Error('Compression requires a scratch file descriptor');
    return descriptor.file;
  }
}
