// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
import { runtime } from './runtime';

export interface JavaOptions {
  runtimeBase?: string;
  licenseKey?: string;
  profileBridge?: boolean;
}
interface Pending { resolve(): void; reject(error: Error): void }

/** A reusable JVM with a fresh bundle class loader for every run. One session may lease it at a time. */
export class JavaRuntime {
  readonly runtimeBase: string;
  private worker?: Worker;
  private ready?: Promise<void>;
  private readonly pending = new Map<number, Pending>();
  private next = 1;
  private leased = false;
  private closed = false;
  private onFailure?: (error: Error) => void;

  constructor(private readonly options: JavaOptions = {}) {
    this.runtimeBase = new URL(options.runtimeBase ?? import.meta.env.BASE_URL + runtime.base, location.href).href;
  }

  /** Downloads and initializes Java without loading any bundle code. */
  warmup(): Promise<void> {
    if (this.closed) return Promise.reject(new Error('Java runtime is closed'));
    if (this.ready) return this.ready;
    const worker = new Worker(new URL('./jvm.worker.ts', import.meta.url));
    this.worker = worker;
    worker.onerror = event => this.failed(new Error(event.message || 'Java worker crashed'));
    worker.onmessage = event => {
      const reply = event.data as { id: number; error?: string };
      const pending = this.pending.get(reply.id);
      if (!pending) return;
      this.pending.delete(reply.id);
      reply.error ? pending.reject(new Error(reply.error)) : pending.resolve();
    };
    this.ready = this.send('init', { licenseKey: this.options.licenseKey, profileBridge: this.options.profileBridge, runtimeBase: this.runtimeBase });
    // A failed warmup can be retried with a fresh worker, never a partial JVM.
    void this.ready.catch(error => { if (this.worker === worker) this.failed(error); });
    return this.ready;
  }

  acquire(onFailure: (error: Error) => void): void {
    if (this.closed) throw new Error('Java runtime is closed');
    if (this.leased) throw new Error('Java runtime is already in use by another session');
    this.leased = true;
    this.onFailure = onFailure;
  }

  async connect(port: MessagePort, buffer: SharedArrayBuffer): Promise<void> {
    if (!this.leased) throw new Error('Java runtime has no session');
    await this.warmup();
    await this.send('connect', { port, buffer }, [port]);
  }

  async release(discard: boolean): Promise<void> {
    this.onFailure = undefined;
    try {
      if (discard) this.reset(new Error('Java execution was interrupted'));
      else if (this.worker) await this.send('disconnect');
    } catch (error) {
      this.reset(error instanceof Error ? error : new Error(String(error)));
      throw error;
    } finally { this.leased = false; }
  }

  dispose(): void {
    this.closed = true;
    this.failed(new Error('Java runtime is closed'));
  }

  private send(type: string, data: Record<string, unknown> = {}, transfer: Transferable[] = []): Promise<void> {
    const worker = this.worker;
    if (!worker) return Promise.reject(new Error('Java runtime is unavailable'));
    const id = this.next++;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Java runtime ${type} timed out`));
      }, type === 'init' ? 180_000 : 10_000);
      this.pending.set(id, {
        resolve: () => { clearTimeout(timeout); resolve(); },
        reject: error => { clearTimeout(timeout); reject(error); },
      });
      worker.postMessage({ type, id, ...data }, transfer);
    });
  }

  private failed(error: Error): void {
    this.reset(error);
    this.onFailure?.(error);
  }

  private reset(error: Error): void {
    this.worker?.terminate(); this.worker = undefined; this.ready = undefined;
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
  }
}
