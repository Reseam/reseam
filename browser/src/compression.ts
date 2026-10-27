// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { type Channel, link } from './channel';
import type { CompressionInit, Endpoint, StorageRequest } from './protocol';
import { type WorkerEvents, WorkerRpc } from './rpc';
import { loadRuntime } from './runtime';

/** The workers that compress DEX entries for one session, each with its own storage connection. */
export class CompressionWorkers {
  private readonly workers: WorkerRpc<never>[] = [];
  private readonly channels: Channel[] = [];
  private closed = false;

  /** Starts `count` workers and returns the endpoints the engine submits jobs to. */
  async start(count: number, runtimeBase: string, storage: WorkerRpc<StorageRequest>, events: WorkerEvents): Promise<Endpoint[]> {
    const module = await WebAssembly.compile(await loadRuntime(runtimeBase, 'reseam_browser_compression.wasm'));
    const endpoints: Endpoint[] = [];
    for (let index = 0; index < count; index++) {
      if (this.closed) throw new Error('Browser patch session is closed');
      const worker = new WorkerRpc<never>(new Worker(new URL('./compression.worker.ts', import.meta.url)), events);
      this.workers.push(worker);
      const files = link(), jobs = link();
      this.channels.push(files.channel, jobs.channel);
      await storage.call({ type: 'connect', endpoint: files.server }, [files.server.port]);
      const init: CompressionInit = { type: 'init', module, storage: files.client, jobs: jobs.server };
      await worker.start(init, [files.client.port, jobs.server.port]);
      endpoints.push(jobs.client);
    }
    return endpoints;
  }

  close(reason: Error): void {
    this.closed = true;
    for (const channel of this.channels) channel.close();
    for (const worker of this.workers) worker.terminate(reason);
  }
}
