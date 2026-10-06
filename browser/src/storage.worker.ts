// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Channel, encode } from './channel';
interface AccessHandle {
  read(buffer: Uint8Array, options: { at: number }): number;
  write(buffer: Uint8Array, options: { at: number }): number;
  truncate(size: number): void;
  flush(): void;
  close(): void;
}
const handles = new Map<string, AccessHandle>();
let root: FileSystemDirectoryHandle;
let directory: FileSystemDirectoryHandle;
let name: string;
let channel: Channel;
let port: MessagePort;
let stopped = false;
let releaseLock: (() => void) | undefined;
async function protectSession(): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    void navigator.locks.request(name, async () => {
      const released = new Promise<void>(done => { releaseLock = done; });
      resolve();
      await released;
    }).catch(reject);
  });
  // A tab crash releases its lock. Reclaim only sessions with no living owner.
  for await (const [entry] of (root as FileSystemDirectoryHandle & { entries(): AsyncIterableIterator<[string, FileSystemHandle]> }).entries()) {
    if (entry === name || !/^reseam-session-[a-f0-9-]{36}$/.test(entry)) continue;
    await navigator.locks.request(entry, { ifAvailable: true }, async lock => {
      if (lock) await root.removeEntry(entry, { recursive: true });
    });
  }
}
async function handle(operation: string, args: Record<string, unknown>): Promise<unknown> {
  const id = args.id as string;
  if (id && !/^[a-f0-9-]{36}$/.test(id)) throw new Error('Invalid storage identity');
  switch (operation) {
    case 'create': {
      const file = await directory.getFileHandle(id, { create: true });
      const access = await (file as FileSystemFileHandle & { createSyncAccessHandle(): Promise<AccessHandle> }).createSyncAccessHandle();
      handles.set(id, access); return null;
    }
    case 'read': {
      const data = new Uint8Array(Math.min(args.size as number, 16 << 20));
      const count = handles.get(id)!.read(data, { at: args.offset as number }); return data.subarray(0, count);
    }
    case 'write': return handles.get(id)!.write(args.data as Uint8Array, { at: args.offset as number });
    case 'truncate': handles.get(id)!.truncate(args.size as number); return null;
    case 'sync': handles.get(id)!.flush(); return null;
    case 'remove': {
      handles.get(id)?.close(); handles.delete(id);
      await directory.removeEntry(id); return null;
    }
    case 'artifact': {
      handles.get(id)?.flush(); handles.get(id)?.close(); handles.delete(id);
      return await (await directory.getFileHandle(id)).getFile();
    }
    case 'dispose': {
      stopped = true;
      for (const handle of handles.values()) handle.close();
      handles.clear();
      if (directory) await root.removeEntry(name, { recursive: true });
      releaseLock?.(); return null;
    }
    default: throw new Error(`Unknown storage operation ${operation}`);
  }
}
function connect(port: MessagePort, channel: Channel): void {
  port.onmessage = async message => {
    if (stopped) return;
    try {
      const value = await handle(message.data.operation, message.data.args);
      // File artifacts use structured clone rather than copying their bytes.
      if (message.data.operation === 'artifact') port.postMessage({ type: 'artifact', value });
      else channel.sendSync(4, encode({ value }));
    } catch (error) {
      if (Atomics.load(channel.control, 3)) return;
      channel.sendSync(4, encode({ error: String(error), quota: error instanceof DOMException && error.name === 'QuotaExceededError' }));
    }
  };
}
self.onmessage = async event => {
  if (event.data.type === 'connect') {
    connect(event.data.port, new Channel(event.data.buffer));
    self.postMessage({ type: 'result', id: event.data.id });
    return;
  }
  if (event.data.type !== 'init') {
    try {
      const value = await handle(event.data.type, { id: event.data.storageId });
      self.postMessage({ type: 'result', id: event.data.id, value });
    } catch (error) { self.postMessage({ type: 'error', id: event.data.id, error: String(error) }); }
    return;
  }
  try {
    ({ name, port } = event.data);
    channel = new Channel(event.data.buffer);
    root = await navigator.storage.getDirectory();
    await protectSession();
    directory = await root.getDirectoryHandle(name, { create: true });
    connect(port, channel);
    self.postMessage({ type: 'ready' });
  } catch (error) { self.postMessage({ type: 'error', error: String(error) }); }
};
