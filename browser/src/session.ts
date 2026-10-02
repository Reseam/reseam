// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
import { Channel } from './channel';
import { loadRuntime } from './runtime';
import type { CompressionConnection } from './compression';
import { JavaRuntime } from './java';
export { JavaRuntime } from './java';

export interface MountedFile { name: string; file: File; directory?: 'input' | 'identity' }
export interface Artifact { name: string; file: File }
export interface SessionOptions {
  javaRuntime?: JavaRuntime;
  compressionWorkers?: number;
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
interface Reply { type: string; id?: number; value?: unknown; error?: string | { problem: EngineProblem; message: string }; files?: { name: string; id: string }[]; fatal?: boolean; wasmMemoryBytes?: number; compressionMemoryBytes?: number }
interface Pending { resolve(value: Reply): void; reject(error: Error): void }

export class BrowserSession {
  private readonly engine = new Worker(new URL('./engine.worker.ts', import.meta.url));
  private readonly java: JavaRuntime;
  private javaLeased = false;
  private readonly storage = new Worker(new URL('./storage.worker.ts', import.meta.url));
  private readonly compressionWorkers: Worker[] = [];
  private readonly compressionChannels: Channel[] = [];
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
  compressionMemoryBytes = 0;
  private constructor(private readonly options: SessionOptions) { this.java = options.javaRuntime ?? new JavaRuntime(options); }

  static async open(files: MountedFile[], options: SessionOptions = {}): Promise<BrowserSession> {
    if (!isSecureContext || !crossOriginIsolated || typeof SharedArrayBuffer === 'undefined') {
      throw new Error('Browser patching requires HTTPS and Cross-Origin-Opener-Policy: same-origin plus Cross-Origin-Embedder-Policy: require-corp.');
    }
    if (!navigator.storage?.getDirectory || !navigator.locks) throw new Error('This browser does not support the storage and locks needed for patching.');
    options.signal?.throwIfAborted();
    validateFiles(files);
    if (options.compressionWorkers !== undefined && (!Number.isInteger(options.compressionWorkers) || options.compressionWorkers < 1 || options.compressionWorkers > 4)) throw new Error('Choose between one and four compression workers');
    const session = new BrowserSession(options);
    const aborted = () => { void session.dispose(new Error('Patching was cancelled')).catch(error => options.onLog?.(String(error))); };
    options.signal?.addEventListener('abort', aborted, { once: true });
    session.removeAbort = () => options.signal?.removeEventListener('abort', aborted);
    const storageLink = new MessageChannel();
    const jvmLink = new MessageChannel();
    const runtimeBase = session.java.runtimeBase;
    try {
      if (options.runtimeBase && new URL(options.runtimeBase, location.href).href !== runtimeBase) throw new Error('Java and engine runtime assets must match');
      session.java.acquire(error => { void session.dispose(error).catch(error => options.onLog?.(String(error))); });
      session.javaLeased = true;
      await session.initialize(session.storage, { name: session.name, port: storageLink.port1, buffer: session.storageChannel.buffer }, [storageLink.port1]);
      const compression: CompressionConnection[] = [];
      const module = await WebAssembly.compile(await loadRuntime(runtimeBase, 'reseam_browser_compression.wasm'));
      const count = options.compressionWorkers ?? Math.min(2, Math.max(1, (navigator.hardwareConcurrency || 2) - 1));
      for (let index = 0; index < count; index++) {
        if (session.disposed) throw new Error('Browser patch session is closed');
        const worker = new Worker(new URL('./compression.worker.ts', import.meta.url));
        session.compressionWorkers.push(worker);
        const storageLink = new MessageChannel(), jobs = new MessageChannel();
        const storageChannel = new Channel(), jobsChannel = new Channel();
        session.compressionChannels.push(storageChannel, jobsChannel);
        await session.send(session.storage, 'connect', { port: storageLink.port1, buffer: storageChannel.buffer }, [storageLink.port1]);
        await session.initialize(worker, { module, storagePort: storageLink.port2, storageBuffer: storageChannel.buffer, port: jobs.port1, buffer: jobsChannel.buffer }, [storageLink.port2, jobs.port1]);
        compression.push({ port: jobs.port2, buffer: jobsChannel.buffer });
      }
      await session.initialize(session.engine, { storagePort: storageLink.port2, storageBuffer: session.storageChannel.buffer, jvmPort: jvmLink.port1, jvmBuffer: session.jvmChannel.buffer, files, runtimeBase, traceNative: options.traceNative, compression }, [storageLink.port2, jvmLink.port1, ...compression.map(slot => slot.port)]);
      session.initializeJvm = () => session.java.connect(jvmLink.port2, session.jvmChannel.buffer);
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
          if (reply.compressionMemoryBytes) this.compressionMemoryBytes = reply.compressionMemoryBytes;
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
  private send(worker: Worker, type: string, data: Record<string, unknown> = {}, transfer: Transferable[] = []): Promise<Reply> {
    if (this.disposed) return Promise.reject(new Error('Browser patch session is closed'));
    const id = this.next++;
    return new Promise((resolve, reject) => { this.pending.set(id, { resolve, reject }); worker.postMessage({ type, id, ...data }, transfer); });
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
  get completed(): boolean { return this.patched; }
  warmup(): Promise<void> {
    if (this.disposed) return Promise.reject(new Error('Browser patch session is closed'));
    return this.java.warmup();
  }
  async mount(files: MountedFile[]): Promise<void> {
    if (this.running || this.patched) throw new Error('Files can only be mounted before patching');
    validateFiles(files);
    await this.send(this.engine, 'mount', { files });
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
    this.engine.terminate();
    for (const channel of this.compressionChannels) channel.close();
    for (const worker of this.compressionWorkers) worker.terminate();
    this.jvmChannel.close(); this.storageChannel.close();
    const javaReleased = this.javaLeased ? this.java.release(this.running).finally(() => {
      this.javaLeased = false;
      if (!this.options.javaRuntime) this.java.dispose();
    }) : Promise.resolve();
    const id = this.next++;
    try {
      const results = await Promise.allSettled([javaReleased, new Promise<Reply>((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Could not clean browser scratch storage')), 10_000);
        this.pending.set(id, {
          resolve: value => { clearTimeout(timeout); resolve(value); },
          reject: error => { clearTimeout(timeout); reject(error); },
        });
        this.storage.postMessage({ type: 'dispose', id });
      })]);
      const errors = results.filter((result): result is PromiseRejectedResult => result.status === 'rejected').map(result => result.reason);
      if (errors.length) throw new AggregateError(errors, 'Could not close browser patch session');
    } finally { this.pending.delete(id); this.storage.terminate(); }
  }
}

function validateFiles(files: MountedFile[]): void {
  const paths = new Set<string>();
  for (const input of files) {
    if (input.directory && input.directory !== 'input' && input.directory !== 'identity') throw new Error('Invalid input directory');
    if (!input.name || /[\\\0]/.test(input.name) || input.name.split('/').some(part => !part || part === '.' || part === '..')) throw new Error('Invalid input filename');
    const path = `${input.directory ?? 'input'}/${input.name}`;
    if (paths.has(path)) throw new Error('Input filenames must be unique');
    paths.add(path);
  }
}
