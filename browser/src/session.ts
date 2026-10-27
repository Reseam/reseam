// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { link } from './channel';
import { CompressionWorkers } from './compression';
import { JavaRuntime } from './java';
import type { InspectRequest, InspectResponse, PatchOutcome, PatchRequest, RunEvent } from './models';
import { validateMounts } from './paths';
import type { EngineInit, EngineOperation, EngineRequest, MountedFile, StorageRequest, StoredFile } from './protocol';
import { type WorkerEvents, WorkerRpc } from './rpc';

export type { MountedFile } from './protocol';
export interface Artifact { name: string; file: File }
export interface SessionOptions {
  javaRuntime?: JavaRuntime;
  compressionWorkers?: number;
  runtimeBase?: string;
  profileBridge?: boolean;
  licenseKey?: string;
  signal?: AbortSignal;
  onEvent?: (event: RunEvent) => void;
  onLog?: (message: string) => void;
}

const closed = () => new Error('Browser patch session is closed');

/** One patch run: an engine worker, its scratch storage and compression workers, and a leased JVM. */
export class BrowserSession {
  private readonly java: JavaRuntime;
  private readonly events: WorkerEvents;
  private readonly engine: WorkerRpc<EngineRequest>;
  private readonly storage: WorkerRpc<StorageRequest>;
  private readonly compression = new CompressionWorkers();
  private readonly storageLink = link();
  private readonly jvmLink = link();
  private readonly name = `reseam-session-${crypto.randomUUID()}`;
  private javaLeased = false;
  private disposing?: Promise<void>;
  private jvmReady?: Promise<void>;
  private running = false;
  private patched = false;
  wasmMemoryBytes = 0;
  compressionMemoryBytes = 0;

  private constructor(private readonly options: SessionOptions) {
    this.java = options.javaRuntime ?? new JavaRuntime(options);
    this.events = {
      onFailure: error => this.disposeLogged(error),
      onLog: options.onLog,
      onEvent: options.onEvent,
      onMemory: (wasm, compression) => { this.wasmMemoryBytes = wasm; this.compressionMemoryBytes = compression; },
    };
    this.engine = new WorkerRpc(new Worker(new URL('./engine.worker.ts', import.meta.url)), this.events);
    this.storage = new WorkerRpc(new Worker(new URL('./storage.worker.ts', import.meta.url)), this.events);
  }

  static async open(files: MountedFile[], options: SessionOptions = {}): Promise<BrowserSession> {
    if (!isSecureContext || !crossOriginIsolated || typeof SharedArrayBuffer === 'undefined') {
      throw new Error('Browser patching requires HTTPS and Cross-Origin-Opener-Policy: same-origin plus Cross-Origin-Embedder-Policy: require-corp.');
    }
    if (!navigator.storage?.getDirectory || !navigator.locks) throw new Error('This browser does not support the storage and locks needed for patching.');
    options.signal?.throwIfAborted();
    validateMounts(files);
    const workers = options.compressionWorkers ?? Math.min(2, Math.max(1, (navigator.hardwareConcurrency || 2) - 1));
    if (!Number.isInteger(workers) || workers < 1 || workers > 4) throw new Error('Choose between one and four compression workers');
    const session = new BrowserSession(options);
    const aborted = () => session.disposeLogged(new Error('Patching was cancelled'));
    options.signal?.addEventListener('abort', aborted, { once: true });
    session.removeAbort = () => options.signal?.removeEventListener('abort', aborted);
    try {
      await session.start(files, workers);
      return session;
    } catch (error) { await session.dispose().catch(cleanup => options.onLog?.(String(cleanup))); throw error; }
  }
  private removeAbort = () => {};

  private async start(files: MountedFile[], workers: number): Promise<void> {
    const runtimeBase = this.java.runtimeBase;
    if (this.options.runtimeBase && new URL(this.options.runtimeBase, location.href).href !== runtimeBase) throw new Error('Java and engine runtime assets must match');
    this.java.acquire(error => this.disposeLogged(error));
    this.javaLeased = true;
    await this.storage.start({ type: 'init', name: this.name, endpoint: this.storageLink.server }, [this.storageLink.server.port]);
    const compression = await this.compression.start(workers, runtimeBase, this.storage, this.events);
    const init: EngineInit = { type: 'init', storage: this.storageLink.client, jvm: this.jvmLink.client, compression, files, runtimeBase };
    await this.engine.start(init, [this.storageLink.client.port, this.jvmLink.client.port, ...compression.map(endpoint => endpoint.port)]);
    this.options.onLog?.('Runtime workers ready');
  }

  inspect(request: InspectRequest): Promise<InspectResponse> { return this.run({ operation: 'inspect', request }); }
  patch(request: PatchRequest): Promise<PatchOutcome> { return this.run({ operation: 'patch', request }); }
  private async run<T>(request: EngineOperation): Promise<T> {
    if (this.disposing) throw closed();
    if (this.running) throw new Error('A browser operation is already running');
    if (this.patched) throw new Error('Open a new session for another patch run');
    this.running = true;
    try {
      if (request.operation === 'patch') {
        this.options.onLog?.('Loading Java runtime for patch execution');
        this.jvmReady ??= this.java.connect(this.jvmLink.server);
        await this.jvmReady;
      }
      const value = await this.engine.call<T>({ type: 'request', request });
      if (request.operation === 'patch') this.patched = true;
      return value;
    } finally { this.running = false; }
  }
  get completed(): boolean { return this.patched; }
  warmup(): Promise<void> {
    if (this.disposing) return Promise.reject(closed());
    return this.java.warmup();
  }
  async mount(files: MountedFile[]): Promise<void> {
    if (this.running || this.patched) throw new Error('Files can only be mounted before patching');
    validateMounts(files);
    await this.engine.call({ type: 'mount', files });
  }
  async artifacts(): Promise<Artifact[]> {
    const files = await this.engine.call<StoredFile[]>({ type: 'artifacts' });
    const artifacts: Artifact[] = [];
    for (const { name, storageId } of files) {
      artifacts.push({ name, file: await this.storage.call<File>({ type: 'artifact', storageId }) });
    }
    return artifacts;
  }
  dispose(reason = new Error('Browser patch session closed')): Promise<void> {
    this.disposing ??= this.close(reason);
    return this.disposing;
  }
  private disposeLogged(reason: Error): void {
    void this.dispose(reason).catch(error => this.options.onLog?.(String(error)));
  }
  private async close(reason: Error): Promise<void> {
    this.removeAbort();
    this.engine.terminate(reason);
    this.compression.close(reason);
    this.jvmLink.channel.close(); this.storageLink.channel.close();
    this.storage.fail(reason);
    const javaReleased = this.javaLeased ? this.java.release(this.running).finally(() => {
      this.javaLeased = false;
      if (!this.options.javaRuntime) this.java.dispose();
    }) : Promise.resolve();
    try {
      const results = await Promise.allSettled([javaReleased, this.storage.call({ type: 'dispose' }, [], 10_000)]);
      const errors = results.filter((result): result is PromiseRejectedResult => result.status === 'rejected').map(result => result.reason);
      if (errors.length) throw new AggregateError(errors, 'Could not close browser patch session');
    } finally { this.storage.terminate(reason); }
  }
}
