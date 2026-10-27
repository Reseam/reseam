// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Channel } from './channel';
import { decode } from './envelope';
import { type Endpoint, PacketKind, type StorageOperation, type StorageReply } from './protocol';

export class StorageFailure extends Error {
  constructor(message: string, readonly quota: boolean) { super(message); }
}

/** Synchronous access to the scratch files the storage worker keeps in OPFS. */
export class StorageClient {
  private readonly reads = new ReadCache<string>();
  private readonly port: MessagePort;
  private readonly channel: Channel;
  constructor(endpoint: Endpoint) {
    this.port = endpoint.port;
    this.channel = new Channel(endpoint.buffer);
  }
  read(id: string, length: number, size: number, offset: number): Uint8Array {
    return this.reads.read(id, length, size, offset, (start, end) =>
      this.call<Uint8Array>({ operation: 'read', id, offset: start, size: end - start }));
  }
  invalidate(id: string): void { this.reads.invalidate(id); }
  call<T = null>(operation: StorageOperation): T {
    this.port.postMessage(operation);
    const packet = this.channel.receiveSync();
    if (packet.kind !== PacketKind.StorageReply) throw new Error('Unexpected storage reply');
    const reply = decode<StorageReply>(packet.bytes);
    if ('error' in reply) throw new StorageFailure(reply.error, reply.quota);
    return reply.value as T;
  }
}

// ZIP readers repeatedly request nearby headers and short ranges. Each cache
// has an 8 MiB limit across files, including apps with many split APKs.
class ReadCache<K> {
  private pages: { key: K; offset: number; bytes: Uint8Array }[] = [];
  invalidate(key: K): void { this.pages = this.pages.filter(page => page.key !== key); }
  read(key: K, length: number, size: number, offset: number, fetch: (start: number, end: number) => Uint8Array): Uint8Array {
    const pageSize = 512 * 1024;
    const start = Math.floor(offset / pageSize) * pageSize;
    const end = Math.min(offset + size, length);
    if (offset >= end) return new Uint8Array();
    if (size > 128 * 1024 || end > start + pageSize) {
      return fetch(offset, end);
    }
    const index = this.pages.findIndex(page => page.key === key && page.offset === start);
    const page = index >= 0 ? this.pages.splice(index, 1)[0] : {
      key, offset: start, bytes: fetch(start, Math.min(start + pageSize, length)),
    };
    if (page.bytes.length !== Math.min(pageSize, length - start)) return fetch(offset, end);
    this.pages.unshift(page);
    if (this.pages.length > 16) this.pages.pop();
    return page.bytes.subarray(offset - start, end - start);
  }
}

export class InputReader {
  private readonly reader = new FileReaderSync();
  private readonly reads = new ReadCache<File>();
  read(file: File, size: number, offset: number): Uint8Array {
    return this.reads.read(file, file.size, size, offset, (start, end) =>
      new Uint8Array(this.reader.readAsArrayBuffer(file.slice(start, end))));
  }
}
