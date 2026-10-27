// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { InspectRequest, PatchRequest, RunEvent, SdkError } from './models';

/** What a shared-memory channel packet carries. */
export enum PacketKind { NativeCall = 1, NativeResult, HostResult, StorageReply, CompressionReply }

export type InputDirectory = 'input' | 'identity';
export interface MountedFile { name: string; file: File; directory?: InputDirectory }
/** A message port for requests and the shared buffer of the channel that answers them. */
export interface Endpoint { port: MessagePort; buffer: SharedArrayBuffer }

/** A request that a worker answers with a reply carrying the same id. */
export type Call<Request> = Request & { id: number };
export type WorkerReply =
  | { type: 'ready' }
  | { type: 'log'; message: string }
  | { type: 'event'; event: RunEvent }
  | { type: 'memory'; wasmBytes: number; compressionBytes: number }
  | { type: 'result'; id: number; value?: unknown }
  | { type: 'error'; id?: number; error: string | SdkError; fatal?: boolean };

export interface EngineInit {
  type: 'init'; storage: Endpoint; jvm: Endpoint; compression: Endpoint[]; files: MountedFile[]; runtimeBase: string;
}
export type EngineOperation = { operation: 'inspect'; request: InspectRequest } | { operation: 'patch'; request: PatchRequest };
export type EngineRequest = { type: 'mount'; files: MountedFile[] } | { type: 'request'; request: EngineOperation } | { type: 'artifacts' };
/** An output file and the scratch file holding it. */
export interface StoredFile { name: string; storageId: string }

export interface StorageInit { type: 'init'; name: string; endpoint: Endpoint }
export type StorageRequest = { type: 'connect'; endpoint: Endpoint } | { type: 'artifact'; storageId: string } | { type: 'dispose' };
export type StorageOperation =
  | { operation: 'create' | 'sync' | 'remove'; id: string }
  | { operation: 'read'; id: string; offset: number; size: number }
  | { operation: 'write'; id: string; offset: number; data: Uint8Array }
  | { operation: 'truncate'; id: string; size: number };
export type StorageReply = { value: unknown } | { error: string; quota: boolean };

export interface CompressionInit { type: 'init'; module: WebAssembly.Module; storage: Endpoint; jobs: Endpoint }
export interface CompressionJob { input: string; output: string; length: bigint; name: string; level: bigint }
export type CompressionReply = { size: bigint; memoryBytes: number } | { error: string };

export type JvmRequest =
  | { type: 'init'; runtimeBase: string; licenseKey?: string; profileBridge?: boolean }
  | { type: 'connect'; endpoint: Endpoint }
  | { type: 'disconnect' };
export interface Declaration { class: string; owner: string; member: string; kind: 'field' | 'method'; id: string }
type Phase = 'execute' | 'finalize';
/** A patch runtime request from the engine, as `crates/patcher/src/bridge/runtime/browser.rs` sends it. */
export type HostRequest =
  | { operation: 'load'; jars: string[]; bundle: string; declarations: Declaration[] }
  | { operation: 'invoke'; handle: number; patch: string; phase: Phase }
  | { operation: 'savepoint'; handle: number; action: 'open' | 'release' | 'rollback'; savepoint: number }
  | { operation: 'close'; handle: number };
/** A host request as the JVM worker receives it: loads carry the jar bytes, invocations the engine revision. */
export type HostCall =
  | { operation: 'load'; payloads: { path: string; bytes: Uint8Array }[]; bundle: string; declarations: Declaration[] }
  | { operation: 'invoke'; handle: number; patch: string; phase: Phase; revision: bigint }
  | Extract<HostRequest, { operation: 'savepoint' | 'close' }>;
// The engine reads `value` even from a failure.
export type HostResult = { value: unknown } | { error: string; value: null };

export interface NativeCall { name: string; args: unknown[] }
/** How a native call failed: a message for a `RuntimeException`, or a BoltFFI error buffer. */
export enum NativeFailure { Message = 1, ErrorBuffer }
export type NativeResult = { value: unknown; revision: bigint } | { error: Uint8Array; kind: NativeFailure };
