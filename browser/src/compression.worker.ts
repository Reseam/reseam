// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { WASI, ConsoleStdout, File as MemoryFile } from '@bjorn3/browser_wasi_shim';
import { Channel } from './channel';
import { encode } from './envelope';
import { type EngineExports, fn } from './native';
import { type CompressionInit, type CompressionJob, type CompressionReply, PacketKind, type WorkerReply } from './protocol';
import { StorageClient } from './storage-client';
import { DiskDescriptor, DiskFile } from './wasi-files';

const post = (reply: WorkerReply) => self.postMessage(reply);
const INPUT_FD = 3, OUTPUT_FD = 4;

async function initialize(init: CompressionInit): Promise<void> {
  const storage = new StorageClient(init.storage);
  const channel = new Channel(init.jobs.buffer);
  const log = ConsoleStdout.lineBuffered(message => post({ type: 'log', message }));
  const wasi = new WASI(['reseam-compression'], [], [new MemoryFile([]).path_open(0, 0n, 0).fd_obj!, log, log]);
  let exports!: EngineExports;
  let failure: string | undefined;
  const instance = await WebAssembly.instantiate(init.module, {
    wasi_snapshot_preview1: wasi.wasiImport,
    reseam_compression: { error: (pointer: number, length: number) => {
      failure = new TextDecoder().decode(new Uint8Array(exports.memory.buffer, pointer, length));
    } },
  });
  exports = instance.exports as EngineExports;
  wasi.initialize(instance as unknown as Parameters<WASI['initialize']>[0]);
  const compress = (job: CompressionJob): CompressionReply => {
    const name = new TextEncoder().encode(job.name);
    let pointer = 0;
    try {
      failure = undefined;
      const output = new DiskFile(storage, job.output, 0n);
      wasi.fds[INPUT_FD] = new DiskDescriptor(new DiskFile(storage, job.input, job.length), 0);
      wasi.fds[OUTPUT_FD] = new DiskDescriptor(output, 0);
      pointer = Number(fn(exports, 'compression_alloc')(name.length));
      if (!pointer) throw new Error('Compression worker ran out of memory');
      new Uint8Array(exports.memory.buffer, pointer, name.length).set(name);
      if (fn(exports, 'compression_run')(INPUT_FD, OUTPUT_FD, pointer, name.length, job.level) !== 0) throw new Error(failure ?? 'DEX compression failed');
      return { size: output.size, memoryBytes: exports.memory.buffer.byteLength };
    } catch (error) {
      return { error: String(error) };
    } finally {
      if (pointer) fn(exports, 'compression_free')(pointer, name.length);
      wasi.fds.length = INPUT_FD;
    }
  };
  init.jobs.port.onmessage = (event: MessageEvent<CompressionJob>) => channel.sendSync(PacketKind.CompressionReply, encode(compress(event.data)));
}

self.onmessage = async (event: MessageEvent<CompressionInit>) => {
  try { await initialize(event.data); post({ type: 'ready' }); }
  catch (error) { post({ type: 'error', error: String(error) }); }
};
