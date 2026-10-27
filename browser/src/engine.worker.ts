// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { WASI, ConsoleStdout, File as MemoryFile } from '@bjorn3/browser_wasi_shim';
import { CompressionPool } from './compression-pool';
import { parseJson, stringifyJson } from './json';
import { JvmBridge } from './jvm-bridge';
import type { RunEvent, SdkError } from './models';
import { alloc, type EngineExports, fn, free, type NativeMethod, read } from './native';
import type { Call, EngineInit, EngineOperation, EngineRequest, WorkerReply } from './protocol';
import { loadRuntime } from './runtime';
import { StorageClient } from './storage-client';
import { Filesystem } from './wasi-fs';

interface Engine { exports: EngineExports; session: number; files: Filesystem; compression: CompressionPool }
type EngineResponse = { value: unknown } | { error: SdkError };

let engine: Engine | undefined;
const decoder = new TextDecoder();
const post = (reply: WorkerReply) => self.postMessage(reply);

function text(exports: EngineExports, pointer: number, length: number): string {
  return decoder.decode(new Uint8Array(exports.memory.buffer, pointer, length));
}

function cryptoRandom(memory: () => WebAssembly.Memory) {
  return (pointer: number, length: number): number => {
    const bytes = new Uint8Array(memory().buffer, pointer, length);
    for (let offset = 0; offset < length; offset += 65536) bytes.set(crypto.getRandomValues(new Uint8Array(Math.min(65536, length - offset))), offset);
    return 0;
  };
}

async function initialize(init: EngineInit): Promise<Engine> {
  const files = new Filesystem(new StorageClient(init.storage));
  const methods = parseJson<NativeMethod[]>(decoder.decode(await loadRuntime(init.runtimeBase, 'methods.json')));
  const log = ConsoleStdout.lineBuffered(message => post({ type: 'log', message }));
  const wasi = new WASI(['reseam-browser'], ['TMPDIR=/tmp'], [new MemoryFile([]).path_open(0, 0n, 0).fd_obj!, log, log, files.descriptor], { debug: false });
  // Imports run only once the instance below exists.
  let exports!: EngineExports;
  const live = () => exports;
  // Signing always requires cryptographic entropy, including in browsers whose
  // WebAssembly memory is shared. Never accept the shim's Math.random fallback.
  wasi.wasiImport.random_get = cryptoRandom(() => live().memory);
  const compression = new CompressionPool(init.compression, wasi, live);
  const bridge = new JvmBridge(init.jvm, new Map(methods.map(method => [method.name, method])), files, live);
  const module = await WebAssembly.compile(await loadRuntime(init.runtimeBase, 'reseam_sdk_browser.wasm'));
  const instance = await WebAssembly.instantiate(module, {
    wasi_snapshot_preview1: wasi.wasiImport,
    reseam_compression: compression.imports,
    reseam_host: {
      call: (pointer: number, length: number) => bridge.call(pointer, length),
      event: (pointer: number, length: number) => post({ type: 'event', event: parseJson<RunEvent>(text(live(), pointer, length)) }),
    },
  });
  exports = instance.exports as EngineExports;
  wasi.initialize(instance as unknown as Parameters<WASI['initialize']>[0]);
  files.mount(init.files);
  return { exports, session: Number(fn(exports, 'reseam_session_new')()), files, compression };
}

function request({ exports, session, compression }: Engine, operation: EngineOperation): EngineResponse {
  const input = alloc(exports, new TextEncoder().encode(stringifyJson(operation)));
  let output = 0;
  try {
    output = Number(fn(exports, 'reseam_request')(session, input + 8, new DataView(exports.memory.buffer).getUint32(input, true)));
    return parseJson<EngineResponse>(decoder.decode(read(exports, output)));
  } finally {
    free(exports, input); free(exports, output);
    post({ type: 'memory', wasmBytes: exports.memory.buffer.byteLength, compressionBytes: compression.memoryBytes });
  }
}

function handle(engine: Engine, message: Call<EngineRequest>): void {
  switch (message.type) {
    case 'mount': engine.files.mount(message.files); post({ type: 'result', id: message.id }); return;
    case 'artifacts': post({ type: 'result', id: message.id, value: engine.files.artifacts() }); return;
    case 'request': {
      const response = request(engine, message.request);
      post('error' in response ? { type: 'error', id: message.id, error: response.error } : { type: 'result', id: message.id, value: response.value });
    }
  }
}

self.onmessage = async (event: MessageEvent<EngineInit | Call<EngineRequest>>) => {
  const message = event.data;
  try {
    if (message.type === 'init') {
      engine = await initialize(message);
      post({ type: 'ready' });
    } else {
      if (!engine) throw new Error('Browser engine is not initialized');
      handle(engine, message);
    }
  } catch (error) {
    post({ type: 'error', id: message.type === 'init' ? undefined : message.id, error: String(error), fatal: message.type === 'request' });
  }
};
