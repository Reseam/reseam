// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { WASI, ConsoleStdout, File as MemoryFile } from '@bjorn3/browser_wasi_shim';
import { Channel, encode } from './channel';
import { DiskFile, DiskDescriptor, StorageClient } from './storage';
import { type EngineExports, fn } from './native';

self.onmessage = async event => {
  if (event.data.type !== 'init') return;
  const data = event.data;
  try {
    const storage = new StorageClient(data.storagePort, new Channel(data.storageBuffer));
    const channel = new Channel(data.buffer);
    const log = ConsoleStdout.lineBuffered(message => self.postMessage({ type: 'log', message }));
    const wasi = new WASI(['reseam-compression'], [], [new MemoryFile([]).path_open(0, 0n, 0).fd_obj!, log, log]);
    let exports: EngineExports;
    let failure: string | undefined;
    const instance = await WebAssembly.instantiate(data.module as WebAssembly.Module, {
      wasi_snapshot_preview1: wasi.wasiImport,
      reseam_compression: { error: (pointer: number, length: number) => {
        failure = new TextDecoder().decode(new Uint8Array(exports.memory.buffer, pointer, length));
      } },
    });
    exports = instance.exports as EngineExports;
    wasi.initialize(instance as unknown as Parameters<WASI['initialize']>[0]);
    (data.port as MessagePort).onmessage = event => {
      const job = event.data as { input: string; output: string; length: bigint; name: string; level: bigint };
      let pointer = 0;
      const name = new TextEncoder().encode(job.name);
      try {
        failure = undefined;
        const input = new DiskFile(storage, job.input, job.length);
        const output = new DiskFile(storage, job.output, 0n);
        wasi.fds[3] = new DiskDescriptor(input, 0);
        wasi.fds[4] = new DiskDescriptor(output, 0);
        pointer = Number(fn(exports, 'compression_alloc')(name.length));
        if (!pointer) throw new Error('Compression worker ran out of memory');
        new Uint8Array(exports.memory.buffer, pointer, name.length).set(name);
        const result = fn(exports, 'compression_run')(3, 4, pointer, name.length, job.level);
        if (result !== 0) throw new Error(failure ?? 'DEX compression failed');
        channel.sendSync(5, encode({ size: output.size, memoryBytes: exports.memory.buffer.byteLength }));
      } catch (error) {
        channel.sendSync(5, encode({ error: String(error) }));
      } finally {
        if (pointer) fn(exports, 'compression_free')(pointer, name.length);
        wasi.fds.length = 3;
      }
    };
    self.postMessage({ type: 'ready' });
  } catch (error) { self.postMessage({ type: 'error', error: String(error) }); }
};
