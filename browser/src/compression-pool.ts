// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { WASI } from '@bjorn3/browser_wasi_shim';
import { Channel } from './channel';
import { decode } from './envelope';
import type { EngineExports } from './native';
import { type CompressionJob, type CompressionReply, type Endpoint, PacketKind } from './protocol';
import { DiskDescriptor, DiskFile } from './wasi-files';

/** The engine's `reseam_compression` imports: DEX entries compressed by worker slots. */
export class CompressionPool {
  private readonly slots: { port: MessagePort; channel: Channel; output?: DiskFile; name?: string }[];
  memoryBytes = 0;
  constructor(endpoints: Endpoint[], private readonly wasi: WASI, private readonly exports: () => EngineExports) {
    this.slots = endpoints.map(endpoint => ({ port: endpoint.port, channel: new Channel(endpoint.buffer) }));
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
      const job: CompressionJob = { input: input.id, output: output.id, length: input.size, name, level };
      slot.port.postMessage(job);
      return index;
    },
    finish: (ticket: number): number => {
      const slot = this.slots[ticket];
      if (!slot?.output) throw new Error('Unknown compression job');
      const packet = slot.channel.receiveSync();
      if (packet.kind !== PacketKind.CompressionReply) throw new Error('Unexpected compression reply');
      const reply = decode<CompressionReply>(packet.bytes);
      if ('error' in reply) throw new Error(`Compressing ${slot.name}: ${reply.error}`);
      slot.output.writtenExternally(reply.size);
      this.memoryBytes = Math.max(this.memoryBytes, reply.memoryBytes * this.slots.length);
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
