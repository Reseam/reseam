// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Channel } from './channel';
import { encode } from './envelope';
import { type Call, type Endpoint, PacketKind, type StorageInit, type StorageOperation, type StorageReply, type StorageRequest, type WorkerReply } from './protocol';

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
let stopped = false;
let releaseLock: (() => void) | undefined;
const post = (reply: WorkerReply) => self.postMessage(reply);

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

function storageId(id: string): string {
  if (!/^[a-f0-9-]{36}$/.test(id)) throw new Error('Invalid storage identity');
  return id;
}

function handle(id: string): AccessHandle {
  const access = handles.get(storageId(id));
  if (!access) throw new Error('Unknown scratch file');
  return access;
}

async function operate(request: StorageOperation): Promise<unknown> {
  switch (request.operation) {
    case 'create': {
      const file = await directory.getFileHandle(storageId(request.id), { create: true });
      handles.set(request.id, await (file as FileSystemFileHandle & { createSyncAccessHandle(): Promise<AccessHandle> }).createSyncAccessHandle());
      return null;
    }
    case 'read': {
      const data = new Uint8Array(Math.min(request.size, 16 << 20));
      return data.subarray(0, handle(request.id).read(data, { at: request.offset }));
    }
    case 'write': return handle(request.id).write(request.data, { at: request.offset });
    case 'truncate': handle(request.id).truncate(request.size); return null;
    case 'sync': handle(request.id).flush(); return null;
    case 'remove': {
      handles.get(storageId(request.id))?.close(); handles.delete(request.id);
      await directory.removeEntry(request.id); return null;
    }
  }
}

/** Serves synchronous file operations from one engine or compression worker. */
function serve({ port, buffer }: Endpoint): void {
  const channel = new Channel(buffer);
  port.onmessage = async (message: MessageEvent<StorageOperation>) => {
    if (stopped) return;
    let reply: StorageReply;
    try { reply = { value: await operate(message.data) }; }
    catch (error) { reply = { error: String(error), quota: error instanceof DOMException && error.name === 'QuotaExceededError' }; }
    if (!channel.closed) channel.sendSync(PacketKind.StorageReply, encode(reply));
  };
}

async function artifact(id: string): Promise<File> {
  const access = handles.get(storageId(id));
  access?.flush(); access?.close(); handles.delete(id);
  return (await directory.getFileHandle(id)).getFile();
}

async function dispose(): Promise<void> {
  stopped = true;
  for (const access of handles.values()) access.close();
  handles.clear();
  if (directory) await root.removeEntry(name, { recursive: true });
  releaseLock?.();
}

async function request(message: Call<StorageRequest>): Promise<unknown> {
  switch (message.type) {
    case 'connect': serve(message.endpoint); return null;
    case 'artifact': return artifact(message.storageId);
    case 'dispose': return dispose();
  }
}

async function initialize(init: StorageInit): Promise<void> {
  name = init.name;
  root = await navigator.storage.getDirectory();
  await protectSession();
  directory = await root.getDirectoryHandle(name, { create: true });
  serve(init.endpoint);
}

self.onmessage = async (event: MessageEvent<StorageInit | Call<StorageRequest>>) => {
  const message = event.data;
  if (message.type === 'init') {
    try { await initialize(message); post({ type: 'ready' }); }
    catch (error) { post({ type: 'error', error: String(error) }); }
    return;
  }
  try { post({ type: 'result', id: message.id, value: await request(message) }); }
  catch (error) { post({ type: 'error', id: message.id, error: String(error) }); }
};
