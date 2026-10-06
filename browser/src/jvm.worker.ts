// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Channel, decode, encode } from './channel';
import { parseJson } from './json';
import { loadRuntime } from './runtime';
import type { NativeMethod } from './native';

declare function cheerpjInit(options: Record<string, unknown>): Promise<void>;
declare function cheerpjRunLibrary(path: string): Promise<any>;
declare function cheerpOSAddStringFile(path: string, bytes: Uint8Array): void;
declare function cheerpOSRemoveStringFile(path: string): void;
let channel: Channel;
let port: MessagePort | undefined;
let active = false;
let host: any;
let runtimePath: string;
let next = 1;
let revision: BigInt64Array | undefined;
let profiling = false;
let profile: Map<string, { calls: number; milliseconds: number }> | undefined;
async function profiledNative(method: NativeMethod, lib: any, args: unknown[]): Promise<unknown> {
  if (!profile) return native(method, lib, args);
  const started = performance.now();
  try { return await native(method, lib, args); }
  finally {
    const entry = profile.get(method.name) ?? { calls: 0, milliseconds: 0 };
    entry.calls++; entry.milliseconds += performance.now() - started;
    profile.set(method.name, entry);
  }
}
const loaders = new Map<number, { loader: any; jars: string[] }>();
async function native(method: NativeMethod, lib: any, args: unknown[]): Promise<unknown> {
  // Rust cannot mutate while waiting for this worker. Every native response
  // carries its latest revision, scoped to the current execute/finalize call.
  if (revision !== undefined && method.name.endsWith('_1handles_1mutation_1revision_1browser')) return revision;
  const inputs: unknown[] = [];
  for (let i = 0; i < args.length; i++) {
    const type = method.parameters[i];
    if (type === 'Ljava/nio/ByteBuffer;') {
      const lengthIndex = method.bufferLengths[String(i)];
      const length = lengthIndex !== undefined ? Number(args[lengthIndex]) : Number(await (args[i] as any).capacity());
      inputs.push(new Uint8Array(await host.bytes(args[i], length)));
    } else if (type.startsWith('[')) {
      const value = args[i] as ArrayBufferView;
      inputs.push(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
    } else inputs.push(args[i]);
  }
  await channel.send(1, encode({ name: method.name, args: inputs }));
  const packet = await channel.receive();
  if (packet.kind !== 2) throw new Error('Unexpected native response');
  const response = decode<{ value: unknown; error?: Uint8Array; kind?: number; revision?: bigint }>(packet.bytes);
  if (revision !== undefined && response.revision !== undefined) revision[0] = response.revision;
  if (response.error) {
    if (response.kind === 2) {
      const ErrorBuffer = await lib.app.reseam.patch.native.BoltFfiErrorBufferException;
      throw await new ErrorBuffer(new Int8Array(response.error.slice().buffer));
    }
    const RuntimeException = await lib.java.lang.RuntimeException;
    throw await new RuntimeException(new TextDecoder().decode(response.error));
  }
  if (method.returns === '[B' || method.returns === '[J') {
    const bytes = response.value as Uint8Array;
    // CheerpJ's small-array conversion reads the backing buffer from offset zero.
    // Bridge packets contain a header, so Java must receive an owned array.
    return method.returns === '[J' ? new BigInt64Array(bytes.slice().buffer) : new Int8Array(bytes.slice().buffer);
  }
  return response.value;
}
async function operation(request: any): Promise<unknown> {
  switch (request.operation) {
    case 'load': {
      const jars = request.payloads.map(({ path, bytes }: { path: string; bytes: Uint8Array }) => {
        const target = `/str/reseam-${next}-${path.split('/').at(-1)}`;
        cheerpOSAddStringFile(target, bytes); return target;
      });
      const declarations = request.declarations;
      let loader: any;
      try { loader = await host.load(runtimePath, jars,
        declarations.map((d: any) => d.class), declarations.map((d: any) => d.owner),
        declarations.map((d: any) => d.member), declarations.map((d: any) => d.kind),
        declarations.map((d: any) => d.id), request.bundle); }
      catch (error) { for (const path of jars) cheerpOSRemoveStringFile(path); throw error; }
      const handle = next++;
      loaders.set(handle, { loader, jars });
      return { handle, patches: parseJson<any>(await loader.describe()) };
    }
    case 'invoke': {
      const loader = loaders.get(request.handle);
      if (!loader) throw new Error('Browser patch loader is closed');
      revision = new BigInt64Array([BigInt.asIntN(64, request.revision)]);
      const started = performance.now();
      if (profiling) profile = new Map();
      try { await loader.loader.invoke(request.patch, request.phase, revision); return null; }
      finally {
        if (profile) console.log('bridge-profile', JSON.stringify({ patch: request.patch, phase: request.phase, milliseconds: performance.now() - started,
          methods: [...profile].map(([name, stats]) => ({ name, ...stats })).sort((a, b) => b.milliseconds - a.milliseconds) }));
        profile = undefined; revision = undefined;
      }
    }
    case 'close': {
      const loader = loaders.get(request.handle);
      if (loader) {
        loaders.delete(request.handle);
        try { await loader.loader.close(); } finally { for (const path of loader.jars) cheerpOSRemoveStringFile(path); }
      }
      return null;
    }
    default: throw new Error(`Unknown JVM operation ${request.operation}`);
  }
}
self.onmessage = async event => {
  const data = event.data;
  try {
    if (data.type === 'connect') {
      if (port || active || loaders.size) throw new Error('Java runtime still has an active session');
      channel = new Channel(data.buffer);
      port = data.port as MessagePort;
      port.onmessage = async message => {
        if (message.data.type !== 'host') return;
        if (active) throw new Error('Concurrent Java host calls are not supported');
        active = true;
        try { await channel.send(3, encode({ value: await operation(message.data.request) })); }
        catch (error) {
          const reason = error && typeof (error as any).getMessage === 'function' ? await (error as any).getMessage() : String(error);
          await channel.send(3, encode({ error: reason, value: null }));
        } finally { active = false; }
      };
    } else if (data.type === 'disconnect') {
      if (active || loaders.size) throw new Error('Java session did not close all bundle loaders');
      port?.close(); port = undefined; revision = undefined;
    } else if (data.type === 'init') {
      profiling = !!data.profileBridge;
      const base = data.runtimeBase as string;
      const methods: NativeMethod[] = parseJson<NativeMethod[]>(new TextDecoder().decode(await loadRuntime(base, 'methods.json')));
      importScripts('https://cjrtnc.leaningtech.com/4.3/loader.js');
      const natives = Object.fromEntries(methods.map(method => [method.name, (lib: any, ...args: unknown[]) => profiledNative(method, lib, args)]));
      await cheerpjInit({ version: 17, status: 'none', natives, licenseKey: data.licenseKey });
      cheerpOSAddStringFile('/str/browser-host.jar', await loadRuntime(base, 'browser-host.jar'));
      cheerpOSAddStringFile('/str/reseam-runtime.jar', await loadRuntime(base, 'reseam-runtime.jar'));
      const library = await cheerpjRunLibrary('/str/browser-host.jar');
      host = await library.app.reseam.browser.BrowserHost;
      runtimePath = '/str/reseam-runtime.jar';
    } else throw new Error(`Unknown Java runtime operation ${data.type}`);
    self.postMessage({ type: 'result', id: data.id });
  } catch (error) { self.postMessage({ type: 'error', id: data.id, error: String(error) }); }
};
