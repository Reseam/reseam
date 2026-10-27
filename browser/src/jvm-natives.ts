// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Channel } from './channel';
import { decode, encode } from './envelope';
import type { NativeMethod } from './native';
import { type NativeCall, NativeFailure, type NativeResult, PacketKind } from './protocol';

type Lib = any;
type Stats = { calls: number; milliseconds: number };

/** The CheerpJ natives of the patch runtime: each call crosses to the engine worker over the channel. */
export class JvmNatives {
  channel?: Channel;
  /** The engine revision of the running invocation, which native responses keep current. */
  revision?: BigInt64Array;
  private profile?: Map<string, Stats>;
  constructor(private readonly host: () => any, private readonly profiling: boolean) {}

  table(methods: NativeMethod[]): Record<string, (lib: Lib, ...args: unknown[]) => Promise<unknown>> {
    return Object.fromEntries(methods.map(method => [method.name, (lib: Lib, ...args: unknown[]) => this.profiled(method, lib, args)]));
  }

  /** Runs one patch phase with `revision`, logging per-method native call times when profiling. */
  async invoke(label: { patch: string; phase: string }, revision: bigint, run: (revision: BigInt64Array) => Promise<void>): Promise<void> {
    this.revision = new BigInt64Array([BigInt.asIntN(64, revision)]);
    const started = performance.now();
    if (this.profiling) this.profile = new Map();
    try { await run(this.revision); }
    finally {
      if (this.profile) console.log('bridge-profile', JSON.stringify({ ...label, milliseconds: performance.now() - started,
        methods: [...this.profile].map(([name, stats]) => ({ name, ...stats })).sort((a, b) => b.milliseconds - a.milliseconds) }));
      this.profile = undefined; this.revision = undefined;
    }
  }

  private async profiled(method: NativeMethod, lib: Lib, args: unknown[]): Promise<unknown> {
    const profile = this.profile;
    if (!profile) return this.native(method, lib, args);
    const started = performance.now();
    try { return await this.native(method, lib, args); }
    finally {
      const entry = profile.get(method.name) ?? { calls: 0, milliseconds: 0 };
      entry.calls++; entry.milliseconds += performance.now() - started;
      profile.set(method.name, entry);
    }
  }

  private async arguments(method: NativeMethod, args: unknown[]): Promise<unknown[]> {
    const inputs: unknown[] = [];
    for (let i = 0; i < args.length; i++) {
      const type = method.parameters[i];
      if (type === 'Ljava/nio/ByteBuffer;') {
        const lengthIndex = method.bufferLengths[String(i)];
        const length = lengthIndex !== undefined ? Number(args[lengthIndex]) : Number(await (args[i] as any).capacity());
        inputs.push(new Uint8Array(await this.host().bytes(args[i], length)));
      } else if (type.startsWith('[')) {
        const value = args[i] as ArrayBufferView;
        inputs.push(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
      } else inputs.push(args[i]);
    }
    return inputs;
  }

  private async native(method: NativeMethod, lib: Lib, args: unknown[]): Promise<unknown> {
    // Rust cannot mutate while waiting for this worker. Every native response
    // carries its latest revision, scoped to the current execute/finalize call.
    if (this.revision !== undefined && method.name.endsWith('_1handles_1mutation_1revision_1browser')) return this.revision;
    if (!this.channel) throw new Error('Java runtime has no session');
    const call: NativeCall = { name: method.name, args: await this.arguments(method, args) };
    await this.channel.send(PacketKind.NativeCall, encode(call));
    const packet = await this.channel.receive();
    if (packet.kind !== PacketKind.NativeResult) throw new Error('Unexpected native response');
    const response = decode<NativeResult>(packet.bytes);
    if ('error' in response) throw await this.exception(lib, response.error, response.kind);
    if (this.revision !== undefined) this.revision[0] = response.revision;
    if (method.returns === '[B' || method.returns === '[J') {
      const bytes = response.value as Uint8Array;
      // CheerpJ's small-array conversion reads the backing buffer from offset zero.
      // Bridge packets contain a header, so Java must receive an owned array.
      return method.returns === '[J' ? new BigInt64Array(bytes.slice().buffer) : new Int8Array(bytes.slice().buffer);
    }
    return response.value;
  }

  private async exception(lib: Lib, error: Uint8Array, kind: NativeFailure): Promise<unknown> {
    if (kind === NativeFailure.ErrorBuffer) {
      const ErrorBuffer = await lib.app.reseam.patch.native.BoltFfiErrorBufferException;
      return new ErrorBuffer(new Int8Array(error.slice().buffer));
    }
    const RuntimeException = await lib.java.lang.RuntimeException;
    return new RuntimeException(new TextDecoder().decode(error));
  }
}
