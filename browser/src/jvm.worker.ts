// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Channel } from './channel';
import { encode } from './envelope';
import { parseJson } from './json';
import { JvmNatives } from './jvm-natives';
import type { NativeMethod } from './native';
import { type Call, type Endpoint, type HostCall, type HostResult, type JvmRequest, PacketKind, type WorkerReply } from './protocol';
import { loadRuntime } from './runtime';

declare function cheerpjInit(options: Record<string, unknown>): Promise<void>;
declare function cheerpjRunLibrary(path: string): Promise<any>;
declare function cheerpOSAddStringFile(path: string, bytes: Uint8Array): void;
declare function cheerpOSRemoveStringFile(path: string): void;
const RUNTIME_PATH = '/str/reseam-runtime.jar';
let natives: JvmNatives;
let port: MessagePort | undefined;
let active = false;
let host: any;
let next = 1;
const loaders = new Map<number, { loader: any; jars: string[] }>();
const post = (reply: WorkerReply) => self.postMessage(reply);

function loader(handle: number) {
  const loader = loaders.get(handle);
  if (!loader) throw new Error('Browser patch loader is closed');
  return loader;
}

async function operation(request: HostCall): Promise<unknown> {
  switch (request.operation) {
    case 'load': {
      const jars = request.payloads.map(({ path, bytes }) => {
        const target = `/str/reseam-${next}-${path.split('/').at(-1)}`;
        cheerpOSAddStringFile(target, bytes); return target;
      });
      const { declarations } = request;
      let loaded: any;
      try {
        loaded = await host.load(RUNTIME_PATH, jars,
          declarations.map(d => d.class), declarations.map(d => d.owner),
          declarations.map(d => d.member), declarations.map(d => d.kind),
          declarations.map(d => d.id), request.bundle);
      } catch (error) { for (const path of jars) cheerpOSRemoveStringFile(path); throw error; }
      const handle = next++;
      loaders.set(handle, { loader: loaded, jars });
      return { handle, patches: parseJson(await loaded.describe()) };
    }
    case 'invoke': {
      const { loader: invoked } = loader(request.handle);
      await natives.invoke({ patch: request.patch, phase: request.phase }, request.revision, revision => invoked.invoke(request.patch, request.phase, revision));
      return null;
    }
    case 'savepoint': {
      await loader(request.handle).loader.savepoint(request.action, request.savepoint);
      return null;
    }
    case 'close': {
      const closing = loaders.get(request.handle);
      if (closing) {
        loaders.delete(request.handle);
        try { await closing.loader.close(); } finally { for (const path of closing.jars) cheerpOSRemoveStringFile(path); }
      }
      return null;
    }
  }
}

async function hostResult(request: HostCall): Promise<HostResult> {
  try { return { value: await operation(request) }; }
  catch (error) {
    const reason = error && typeof (error as any).getMessage === 'function' ? await (error as any).getMessage() : String(error);
    return { error: reason, value: null };
  }
}

function connect(endpoint: Endpoint): void {
  if (port || active || loaders.size) throw new Error('Java runtime still has an active session');
  const channel = new Channel(endpoint.buffer);
  natives.channel = channel;
  port = endpoint.port;
  port.onmessage = async (message: MessageEvent<HostCall>) => {
    if (active) throw new Error('Concurrent Java host calls are not supported');
    active = true;
    try { await channel.send(PacketKind.HostResult, encode(await hostResult(message.data))); }
    finally { active = false; }
  };
}

async function initialize(request: Extract<JvmRequest, { type: 'init' }>): Promise<void> {
  natives = new JvmNatives(() => host, !!request.profileBridge);
  const base = request.runtimeBase;
  const methods = parseJson<NativeMethod[]>(new TextDecoder().decode(await loadRuntime(base, 'methods.json')));
  importScripts('https://cjrtnc.leaningtech.com/4.3/loader.js');
  await cheerpjInit({ version: 17, status: 'none', natives: natives.table(methods), licenseKey: request.licenseKey });
  cheerpOSAddStringFile('/str/browser-host.jar', await loadRuntime(base, 'browser-host.jar'));
  cheerpOSAddStringFile(RUNTIME_PATH, await loadRuntime(base, 'reseam-runtime.jar'));
  const library = await cheerpjRunLibrary('/str/browser-host.jar');
  host = await library.app.reseam.browser.BrowserHost;
}

async function handle(request: JvmRequest): Promise<void> {
  switch (request.type) {
    case 'init': return initialize(request);
    case 'connect': return connect(request.endpoint);
    case 'disconnect':
      if (active || loaders.size) throw new Error('Java session did not close all bundle loaders');
      port?.close(); port = undefined;
  }
}

self.onmessage = async (event: MessageEvent<Call<JvmRequest>>) => {
  try { await handle(event.data); post({ type: 'result', id: event.data.id }); }
  catch (error) { post({ type: 'error', id: event.data.id, error: String(error) }); }
};
