// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
import { WASI, ConsoleStdout, File as MemoryFile, Directory } from '@bjorn3/browser_wasi_shim';
import { Channel, decode, encode } from './channel';
import { parseJson, stringifyJson } from './json';
import { loadRuntime } from './runtime';
import { DiskDirectory, DiskFile, InputFile, InputReader, RootDescriptor, StorageClient } from './storage';
import { alloc, callNative, EngineExports, fn, free, NativeMethod, read } from './native';

let exports: EngineExports;
let root: DiskDirectory;
let jvm: MessagePort;
let jvmChannel: Channel;
let methods: Map<string, NativeMethod>;
let storage: StorageClient;
let busy = false;
let traceNative = false;
const decoder = new TextDecoder();
function lookup(path: string) {
  const parts = path.split('/').filter(Boolean);
  if (parts.includes('..') || path.includes('\0')) throw new Error('Invalid browser file path');
  let inode = root.contents.get(parts.shift()!);
  for (const part of parts) {
    if (!(inode instanceof Directory)) throw new Error(`No directory in ${path}`);
    inode = inode.contents.get(part);
  }
  if (!inode) throw new Error(`No browser file ${path}`);
  return inode;
}
function readFile(path: string): Uint8Array {
  const file = lookup(path);
  if (!(file instanceof DiskFile || file instanceof InputFile)) throw new Error(`Not a file: ${path}`);
  const length = Number(file.size);
  const data = new Uint8Array(length);
  for (let offset = 0; offset < length;) {
    const bytes = file.read(Math.min(1 << 20, length - offset), BigInt(offset));
    if (!bytes.length) throw new Error(`Unexpected end of ${path}`);
    data.set(bytes, offset); offset += bytes.length;
  }
  return data;
}
function hostCall(pointer: number, length: number): number {
  const request = parseJson<any>(decoder.decode(new Uint8Array(exports.memory.buffer, pointer, length)));
  if (request.operation === 'load') {
    request.payloads = request.jars.map((path: string) => ({ path, bytes: readFile(path) }));
  }
  if (request.operation === 'invoke') request.revision = fn(exports, 'reseam_bridge_revision')();
  jvm.postMessage({ type: 'host', request });
  for (;;) {
    const packet = jvmChannel.receiveSync();
    if (packet.kind === 3) {
      const response = decode(packet.bytes);
      return alloc(exports, new TextEncoder().encode(stringifyJson(response)));
    }
    if (packet.kind !== 1) throw new Error('Unexpected JVM bridge packet');
    try {
      const { name, args } = decode<{ name: string; args: unknown[] }>(packet.bytes);
      const method = methods.get(name);
      if (!method) throw new Error(`Unknown native method ${name}`);
      const result = callNative(exports, method, args) as { value?: unknown; error?: Uint8Array; revision?: bigint };
      if (!result.error) result.revision = BigInt(fn(exports, 'reseam_bridge_revision')());
      if (traceNative && /mutation_1revision|add_1interface|get_1class_1info|find_1class/.test(name)) console.log('native', name.replaceAll('_1', '_'), stringifyJson({ args: args.map(arg => arg instanceof Uint8Array ? [...arg.slice(0, 256)] : arg), result: (result as { value?: unknown }).value instanceof Uint8Array ? [...((result as { value: Uint8Array }).value).slice(0, 512)] : result }));
      jvmChannel.sendSync(2, encode(result));
    } catch (error) { jvmChannel.sendSync(2, encode({ error: new TextEncoder().encode(String(error)), kind: 1 })); }
  }
}
async function initialize(data: Record<string, any>): Promise<void> {
  traceNative = !!data.traceNative;
  storage = new StorageClient(data.storagePort, new Channel(data.storageBuffer));
  jvm = data.jvmPort; jvmChannel = new Channel(data.jvmBuffer);
  methods = new Map(parseJson<NativeMethod[]>(decoder.decode(await loadRuntime(data.runtimeBase, 'methods.json'))).map(method => [method.name, method]));
  root = new DiskDirectory(storage);
  for (const name of ['input', 'tmp', 'output', 'identity']) root.contents.set(name, new DiskDirectory(storage));
  const log = ConsoleStdout.lineBuffered(message => self.postMessage({ type: 'log', message }));
  const wasi = new WASI(['reseam-browser'], ['TMPDIR=/tmp'], [new MemoryFile([]).path_open(0, 0n, 0).fd_obj!, log, log, new RootDescriptor(root)], { debug: false });
  // Signing always requires cryptographic entropy, including in browsers whose
  // WebAssembly memory is shared. Never accept the shim's Math.random fallback.
  wasi.wasiImport.random_get = (pointer: number, length: number) => {
    const bytes = new Uint8Array(exports.memory.buffer, pointer, length);
    for (let offset = 0; offset < length; offset += 65536) {
      const random = crypto.getRandomValues(new Uint8Array(Math.min(65536, length - offset)));
      bytes.set(random, offset);
    }
    return 0;
  };
  const module = await WebAssembly.compile(await loadRuntime(data.runtimeBase, 'reseam_sdk_browser.wasm'));
  const instance = await WebAssembly.instantiate(module, {
    wasi_snapshot_preview1: wasi.wasiImport,
    reseam_host: {
      call: hostCall,
      event: (pointer: number, length: number) => self.postMessage({ type: 'event', event: parseJson<any>(decoder.decode(new Uint8Array(exports.memory.buffer, pointer, length))) }),
    },
  });
  exports = instance.exports as EngineExports;
  wasi.initialize(instance as unknown as Parameters<WASI['initialize']>[0]);
  const inputReader = new InputReader();
  for (const input of data.files as { name: string; file: File; directory?: string }[]) {
    let folder = root.contents.get(input.directory ?? 'input') as Directory;
    const parts = input.name.split('/');
    for (const part of parts.slice(0, -1)) {
      let child = folder.contents.get(part);
      if (!child) { child = new DiskDirectory(storage); folder.contents.set(part, child); }
      if (!(child instanceof Directory)) throw new Error('Input path collides with a file');
      folder = child;
    }
    if (folder.contents.has(parts.at(-1)!)) throw new Error('Duplicate input path');
    folder.contents.set(parts.at(-1)!, new InputFile(input.file, inputReader));
  }
  self.postMessage({ type: 'ready' });
}
self.onmessage = async event => {
  const data = event.data;
  try {
    if (data.type === 'init') { await initialize(data); return; }
    if (data.type === 'request') {
      if (busy) throw new Error('A patch run is already active');
      busy = true;
      try {
        const request = alloc(exports, new TextEncoder().encode(stringifyJson(data.request)));
        let result = 0;
        try {
          result = Number(fn(exports, 'reseam_request')(request + 8, new DataView(exports.memory.buffer).getUint32(request, true)));
          self.postMessage({ type: 'result', id: data.id, wasmMemoryBytes: exports.memory.buffer.byteLength, ...parseJson<any>(decoder.decode(read(exports, result))) });
        } finally { free(exports, request); free(exports, result); }
      } finally { busy = false; }
    }
    if (data.type === 'artifacts') {
      const folder = lookup('/output');
      if (!(folder instanceof Directory)) throw new Error('Output directory is missing');
      const files: { name: string; id: string }[] = [];
      const walk = (directory: Directory, prefix: string) => {
        for (const [name, inode] of directory.contents) {
          if (inode instanceof Directory) walk(inode, prefix + name + '/');
          else if (inode instanceof DiskFile) files.push({ name: prefix + name, id: inode.id });
        }
      };
      walk(folder, '');
      const identity = lookup('/identity') as Directory;
      for (const [name, inode] of identity.contents) if (inode instanceof DiskFile) files.push({ name: `identity/${name}`, id: inode.id });
      self.postMessage({ type: 'artifacts', id: data.id, files });
    }
  } catch (error) { self.postMessage({ type: 'error', id: data.id, error: String(error), fatal: data.type === 'request' }); }
};
