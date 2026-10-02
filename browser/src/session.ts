// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
import { Channel } from './channel';
import { runtime } from './runtime';

export interface MountedFile { name: string; file: File; directory?: 'input' | 'identity' }
export interface Artifact { name: string; file: File }
export interface SessionOptions {
  runtimeBase?: string;
  traceNative?: boolean;
  profileBridge?: boolean;
  licenseKey?: string;
  signal?: AbortSignal;
  onEvent?: (event: unknown) => void;
  onLog?: (message: string) => void;
}
export interface EngineProblem { type: string; [key: string]: unknown }
export class EngineError extends Error {
  constructor(readonly problem: EngineProblem, message: string) { super(message); this.name = 'EngineError'; }
}
function hostError(error: string | { problem: EngineProblem; message: string }): Error {
  return typeof error === 'string' ? new Error(error) : new EngineError(error.problem, error.message);
}
interface Reply { type: string; id?: number; value?: unknown; error?: string | { problem: EngineProblem; message: string }; files?: { name: string; id: string }[]; fatal?: boolean; wasmMemoryBytes?: number }
interface Pending { resolve(value: Reply): void; reject(error: Error): void }

export class BrowserSession {
  private readonly engine = new Worker(new URL('./engine.worker.ts', import.meta.url));
  private readonly jvm = new Worker(new URL('./jvm.worker.ts', import.meta.url));
  private readonly storage = new Worker(new URL('./storage.worker.ts', import.meta.url));
  private readonly storageChannel = new Channel();
  private readonly jvmChannel = new Channel();
  private readonly pending = new Map<number, Pending>();
  private next = 1;
  private disposed = false;
  private disposing?: Promise<void>;
  private readonly initializing = new Set<(error: Error) => void>();
  private readonly name = `reseam-session-${crypto.randomUUID()}`;
  private initializeJvm?: () => Promise<void>;
  private jvmReady?: Promise<void>;
  private running = false;
  private patched = false;
  wasmMemoryBytes = 0;
  private constructor(private readonly options: SessionOptions) {}

  static async open(files: MountedFile[], options: SessionOptions = {}): Promise<BrowserSession> {
    if (!isSecureContext || !crossOriginIsolated || typeof SharedArrayBuffer === 'undefined') {
      throw new Error('Browser patching requires HTTPS and Cross-Origin-Opener-Policy: same-origin plus Cross-Origin-Embedder-Policy: require-corp.');
    }
    if (!navigator.storage?.getDirectory || !navigator.locks) throw new Error('This browser does not support the storage and locks needed for patching.');
    options.signal?.throwIfAborted();
    const paths = new Set<string>();
    for (const input of files) {
      if (input.directory && input.directory !== 'input' && input.directory !== 'identity') throw new Error('Invalid input directory');
      if (!input.name || /[\\\0]/.test(input.name) || input.name.split('/').some(part => !part || part === '.' || part === '..')) throw new Error('Invalid input filename');
      const path = `${input.directory ?? 'input'}/${input.name}`;
      if (paths.has(path)) throw new Error('Input filenames must be unique');
      paths.add(path);
    }
    const session = new BrowserSession(options);
    const aborted = () => { void session.dispose(new Error('Patching was cancelled')).catch(error => options.onLog?.(String(error))); };
    options.signal?.addEventListener('abort', aborted, { once: true });
    session.removeAbort = () => options.signal?.removeEventListener('abort', aborted);
    const storageLink = new MessageChannel();
    const jvmLink = new MessageChannel();
    const runtimeBase = new URL(options.runtimeBase ?? import.meta.env.BASE_URL + runtime.base, location.href).href;
    try {
      await session.initialize(session.storage, { name: session.name, port: storageLink.port1, buffer: session.storageChannel.buffer }, [storageLink.port1]);
      await session.initialize(session.engine, { storagePort: storageLink.port2, storageBuffer: session.storageChannel.buffer, jvmPort: jvmLink.port1, jvmBuffer: session.jvmChannel.buffer, files, runtimeBase, traceNative: options.traceNative }, [storageLink.port2, jvmLink.port1]);
      session.initializeJvm = () => session.initialize(session.jvm, { port: jvmLink.port2, buffer: session.jvmChannel.buffer, runtimeBase, licenseKey: options.licenseKey, traceNative: options.traceNative, profileBridge: options.profileBridge }, [jvmLink.port2]);
      return session;
    } catch (error) { await session.dispose(); throw error; }
  }
  private removeAbort = () => {};
  private initialize(worker: Worker, data: Record<string, unknown>, transfer: Transferable[]): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.disposed) { reject(new Error('Browser patch session is closed')); return; }
      let settled = false;
      const cancel = (error: Error) => finish(error);
      const timeout = setTimeout(() => finish(new Error('Browser runtime initialization timed out')), 180_000);
      const finish = (error?: Error) => {
        if (settled) return;
        settled = true; clearTimeout(timeout); this.initializing.delete(cancel);
        error ? reject(error) : resolve();
      };
      this.initializing.add(cancel);
      worker.onerror = event => { const error = new Error(event.message || 'Browser worker crashed'); finish(error); void this.dispose(error).catch(error => this.options.onLog?.(String(error))); };
      worker.onmessage = event => {
        const reply = event.data as Reply & { event?: unknown; message?: string };
        if (reply.type === 'ready') { this.options.onLog?.('Runtime worker ready'); finish(); return; }
        if (reply.type === 'event') { this.options.onEvent?.(reply.event); return; }
        if (reply.type === 'log') { this.options.onLog?.(reply.message ?? ''); return; }
        if (reply.id !== undefined) {
          if (reply.wasmMemoryBytes) this.wasmMemoryBytes = reply.wasmMemoryBytes;
          const pending = this.pending.get(reply.id);
          this.pending.delete(reply.id);
          if (reply.error) pending?.reject(hostError(reply.error)); else pending?.resolve(reply);
          if (reply.fatal) void this.dispose(hostError(reply.error ?? 'Engine worker failed')).catch(error => this.options.onLog?.(String(error)));
        } else if (reply.type === 'error') { const error = hostError(reply.error ?? 'Browser worker failed'); finish(error); this.fail(error); }
      };
      worker.postMessage({ type: 'init', ...data }, transfer);
    });
  }
  private fail(error: Error): void {
    for (const cancel of this.initializing) cancel(error);
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
  }
  private send(worker: Worker, type: string, data: Record<string, unknown> = {}): Promise<Reply> {
    if (this.disposed) return Promise.reject(new Error('Browser patch session is closed'));
    const id = this.next++;
    return new Promise((resolve, reject) => { this.pending.set(id, { resolve, reject }); worker.postMessage({ type, id, ...data }); });
  }
  async request<T>(operation: 'inspect' | 'patch', request: unknown): Promise<T> {
    if (this.disposed) throw new Error('Browser patch session is closed');
    if (this.running) throw new Error('A browser operation is already running');
    if (this.patched) throw new Error('Open a new session for another patch run');
    this.running = true;
    try {
      if (operation === 'patch') {
        this.options.onLog?.('Loading Java runtime for patch execution');
        this.jvmReady ??= this.initializeJvm!();
        await this.jvmReady;
      }
      const response = await this.send(this.engine, 'request', { request: { operation, request } });
      if (operation === 'patch') this.patched = true;
      return response.value as T;
    } finally { this.running = false; }
  }
  async artifacts(): Promise<Artifact[]> {
    const files = (await this.send(this.engine, 'artifacts')).files!;
    const artifacts: Artifact[] = [];
    for (const item of files) {
      const reply = await this.send(this.storage, 'artifact', { storageId: item.id });
      artifacts.push({ name: item.name, file: reply.value as File });
    }
    return artifacts;
  }
  dispose(reason = new Error('Browser patch session closed')): Promise<void> {
    this.disposing ??= this.close(reason);
    return this.disposing;
  }
  private async close(reason: Error): Promise<void> {
    this.disposed = true;
    this.removeAbort(); this.fail(reason);
    this.engine.terminate(); this.jvm.terminate();
    this.jvmChannel.close(); this.storageChannel.close();
    const id = this.next++;
    try {
      await new Promise<Reply>((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Could not clean browser scratch storage')), 10_000);
        this.pending.set(id, {
          resolve: value => { clearTimeout(timeout); resolve(value); },
          reject: error => { clearTimeout(timeout); reject(error); },
        });
        this.storage.postMessage({ type: 'dispose', id });
      });
    } finally { this.pending.delete(id); this.storage.terminate(); }
  }
}
