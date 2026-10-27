// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Channel } from './channel';
import { decode, encode } from './envelope';
import { parseJson, stringifyJson } from './json';
import { alloc, callNative, type EngineExports, fn, type NativeMethod } from './native';
import { type Endpoint, type HostCall, type HostRequest, type NativeCall, NativeFailure, type NativeResult, PacketKind } from './protocol';
import type { Filesystem } from './wasi-fs';

/**
 * The engine's `reseam_host.call` import. It forwards a patch runtime request to the JVM worker and
 * serves the native calls patch code makes until the JVM sends its result.
 */
export class JvmBridge {
  private readonly port: MessagePort;
  private readonly channel: Channel;
  private readonly decoder = new TextDecoder();
  private readonly encoder = new TextEncoder();
  constructor(endpoint: Endpoint, private readonly methods: Map<string, NativeMethod>, private readonly files: Filesystem, private readonly exports: () => EngineExports) {
    this.port = endpoint.port;
    this.channel = new Channel(endpoint.buffer);
  }

  call(pointer: number, length: number): number {
    const exports = this.exports();
    const request = parseJson<HostRequest>(this.decoder.decode(new Uint8Array(exports.memory.buffer, pointer, length)));
    this.port.postMessage(this.forwarded(request));
    for (;;) {
      const packet = this.channel.receiveSync();
      if (packet.kind === PacketKind.HostResult) return alloc(exports, this.encoder.encode(stringifyJson(decode(packet.bytes))));
      if (packet.kind !== PacketKind.NativeCall) throw new Error('Unexpected JVM bridge packet');
      this.channel.sendSync(PacketKind.NativeResult, encode(this.native(decode<NativeCall>(packet.bytes))));
    }
  }

  private revision(): bigint {
    return BigInt(fn(this.exports(), 'reseam_bridge_revision')());
  }

  private forwarded(request: HostRequest): HostCall {
    switch (request.operation) {
      case 'load': {
        const { jars, ...rest } = request;
        return { ...rest, payloads: jars.map(path => ({ path, bytes: this.files.readFile(path) })) };
      }
      case 'invoke': return { ...request, revision: this.revision() };
      default: return request;
    }
  }

  private native({ name, args }: NativeCall): NativeResult {
    try {
      const method = this.methods.get(name);
      if (!method) throw new Error(`Unknown native method ${name}`);
      const outcome = callNative(this.exports(), method, args);
      return 'error' in outcome ? outcome : { value: outcome.value, revision: this.revision() };
    } catch (error) { return { error: this.encoder.encode(String(error)), kind: NativeFailure.Message }; }
  }
}
