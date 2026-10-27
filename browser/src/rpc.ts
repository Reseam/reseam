// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Problem, RunEvent, SdkError } from './models';
import type { WorkerReply } from './protocol';

/** How long a worker may take to download and compile its runtime. */
export const INIT_TIMEOUT = 180_000;

export class EngineError extends Error {
  constructor(readonly problem: Problem, message: string) { super(message); this.name = 'EngineError'; }
}

export interface WorkerEvents {
  /** The worker crashed or reported a failure it cannot recover from. */
  onFailure(error: Error): void;
  onLog?(message: string): void;
  onEvent?(event: RunEvent): void;
  onMemory?(wasmBytes: number, compressionBytes: number): void;
}
interface Pending { resolve(value: unknown): void; reject(error: Error): void }

function replyError(error: string | SdkError): Error {
  return typeof error === 'string' ? new Error(error) : new EngineError(error.problem, error.message);
}

/** Requests to one worker, each answered by the reply carrying its id. */
export class WorkerRpc<Request extends { type: string }> {
  private readonly pending = new Map<number, Pending>();
  private next = 1;
  private ready?: Pending;
  private closed?: Error;
  constructor(readonly worker: Worker, private readonly events: WorkerEvents) {
    worker.onerror = event => this.crash(new Error(event.message || 'Browser worker crashed'));
    worker.onmessage = event => this.receive(event.data as WorkerReply);
  }

  /** Sends the worker its setup and waits until it reports ready. */
  start<Init extends { type: 'init' }>(init: Init, transfer: Transferable[], timeout = INIT_TIMEOUT): Promise<void> {
    if (this.closed) return Promise.reject(this.closed);
    return new Promise<unknown>((resolve, reject) => {
      this.ready = this.settle(resolve, reject, timeout, 'Browser runtime initialization timed out', () => { this.ready = undefined; });
      this.worker.postMessage(init, transfer);
    }).then(() => {});
  }

  call<T = void>(request: Request, transfer: Transferable[] = [], timeout?: number): Promise<T> {
    if (this.closed) return Promise.reject(this.closed);
    const id = this.next++;
    return new Promise<unknown>((resolve, reject) => {
      this.pending.set(id, this.settle(resolve, reject, timeout, `Browser worker ${request.type} timed out`, () => this.pending.delete(id)));
      this.worker.postMessage({ ...request, id }, transfer);
    }) as Promise<T>;
  }

  /** Rejects every outstanding request. */
  fail(error: Error): void {
    this.ready?.reject(error);
    for (const pending of [...this.pending.values()]) pending.reject(error);
  }

  /** Stops the worker; later requests fail with `reason`. */
  terminate(reason: Error): void {
    this.closed = reason;
    this.fail(reason);
    this.worker.terminate();
  }

  private settle(resolve: (value: unknown) => void, reject: (error: Error) => void, timeout: number | undefined, expired: string, done: () => void): Pending {
    const timer = timeout === undefined ? undefined : setTimeout(() => pending.reject(new Error(expired)), timeout);
    const finish = () => { clearTimeout(timer); done(); };
    const pending: Pending = {
      resolve: value => { finish(); resolve(value); },
      reject: error => { finish(); reject(error); },
    };
    return pending;
  }

  private crash(error: Error): void {
    this.fail(error);
    this.events.onFailure(error);
  }

  private receive(reply: WorkerReply): void {
    switch (reply.type) {
      case 'ready': this.ready?.resolve(undefined); return;
      case 'log': this.events.onLog?.(reply.message); return;
      case 'event': this.events.onEvent?.(reply.event); return;
      case 'memory': this.events.onMemory?.(reply.wasmBytes, reply.compressionBytes); return;
      case 'result': this.pending.get(reply.id)?.resolve(reply.value); return;
      case 'error': {
        const error = replyError(reply.error);
        if (reply.id === undefined) this.fail(error); else this.pending.get(reply.id)?.reject(error);
        if (reply.fatal) this.events.onFailure(error);
      }
    }
  }
}
