// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Endpoint, JvmRequest } from './protocol';
import { INIT_TIMEOUT, WorkerRpc } from './rpc';
import { runtime } from './runtime';

export interface JavaOptions {
  runtimeBase?: string;
  licenseKey?: string;
  profileBridge?: boolean;
}

const REQUEST_TIMEOUT = 10_000;

/** A reusable JVM with a fresh bundle class loader for every run. One session may lease it at a time. */
export class JavaRuntime {
  readonly runtimeBase: string;
  private worker?: WorkerRpc<JvmRequest>;
  private ready?: Promise<void>;
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
    const worker = new WorkerRpc<JvmRequest>(new Worker(new URL('./jvm.worker.ts', import.meta.url)), { onFailure: error => this.failed(error) });
    this.worker = worker;
    const { licenseKey, profileBridge } = this.options;
    this.ready = worker.call({ type: 'init', runtimeBase: this.runtimeBase, licenseKey, profileBridge }, [], INIT_TIMEOUT);
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

  async connect(endpoint: Endpoint): Promise<void> {
    if (!this.leased) throw new Error('Java runtime has no session');
    await this.warmup();
    await this.send({ type: 'connect', endpoint }, [endpoint.port]);
  }

  async release(discard: boolean): Promise<void> {
    this.onFailure = undefined;
    try {
      if (discard) this.reset(new Error('Java execution was interrupted'));
      else if (this.worker) await this.send({ type: 'disconnect' });
    } catch (error) {
      this.reset(error instanceof Error ? error : new Error(String(error)));
      throw error;
    } finally { this.leased = false; }
  }

  dispose(): void {
    this.closed = true;
    this.failed(new Error('Java runtime is closed'));
  }

  private send(request: JvmRequest, transfer: Transferable[] = []): Promise<void> {
    if (!this.worker) return Promise.reject(new Error('Java runtime is unavailable'));
    return this.worker.call(request, transfer, REQUEST_TIMEOUT);
  }

  private failed(error: Error): void {
    this.reset(error);
    this.onFailure?.(error);
  }

  private reset(error: Error): void {
    this.worker?.terminate(error); this.worker = undefined; this.ready = undefined;
  }
}
