// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { WASI } from '@bjorn3/browser_wasi_shim';
import { parseJson, stringifyJson } from './json';
import type {
  ChangesRequest, OptionDeclaration, OptionValue, ParsedOption, PatchChange, PlanRequest, SdkError, SelectionPlan,
  SigningKeyMaterial,
} from './models';
import { EngineError } from './rpc';
import { loadRuntime } from './runtime';

type ToolsExports = {
  memory: WebAssembly.Memory;
  tools_alloc(length: number): number;
  tools_free(pointer: number, length: number): void;
  tools_call(pointer: number, length: number): bigint;
};
type ToolsRequest =
  | { operation: 'plan'; request: PlanRequest }
  | { operation: 'changes'; request: ChangesRequest }
  | { operation: 'parse_option'; request: { option: OptionDeclaration; text: string } }
  | { operation: 'option_text'; request: OptionValue }
  | { operation: 'import_pkcs12'; request: { keystore: number[]; password: string } }
  | { operation: 'export_pkcs12'; request: { material: SigningKeyMaterial; password: string } }
  | { operation: 'certificate_fingerprint'; request: number[] };

/**
 * The engine calls that read no files and run no patch, loaded once and answered synchronously on the
 * calling thread: planning a selection, reading option text, comparing a build with newer patches, and
 * PKCS#12 key backups. Failures throw {@link EngineError}.
 */
export class EngineTools {
  private constructor(private readonly exports: ToolsExports) {}

  static async load(runtimeBase: string): Promise<EngineTools> {
    const wasi = new WASI([], [], [], { debug: false });
    // PKCS#12 backups need real entropy; never accept the shim's Math.random fallback.
    wasi.wasiImport.random_get = (pointer: number, length: number) => {
      crypto.getRandomValues(new Uint8Array(exports.memory.buffer, pointer, length));
      return 0;
    };
    const module = await WebAssembly.compile(await loadRuntime(runtimeBase, 'reseam_browser_tools.wasm'));
    const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasi.wasiImport });
    const exports = instance.exports as unknown as ToolsExports;
    wasi.initialize(instance as unknown as Parameters<WASI['initialize']>[0]);
    return new EngineTools(exports);
  }

  plan(request: PlanRequest): SelectionPlan { return this.call({ operation: 'plan', request }); }
  changes(request: ChangesRequest): PatchChange[] { return this.call({ operation: 'changes', request }); }
  parseOption(option: OptionDeclaration, text: string): ParsedOption {
    return this.call({ operation: 'parse_option', request: { option, text } });
  }
  optionText(value: OptionValue): string { return this.call({ operation: 'option_text', request: value }); }
  importPkcs12(keystore: Uint8Array, password: string): SigningKeyMaterial {
    return this.call({ operation: 'import_pkcs12', request: { keystore: [...keystore], password } });
  }
  exportPkcs12(material: SigningKeyMaterial, password: string): Uint8Array {
    return Uint8Array.from(this.call<number[]>({ operation: 'export_pkcs12', request: { material, password } }));
  }
  certificateFingerprint(certificate: Uint8Array): string {
    return this.call({ operation: 'certificate_fingerprint', request: [...certificate] });
  }

  private call<T>(request: ToolsRequest): T {
    const input = new TextEncoder().encode(stringifyJson(request));
    const pointer = this.exports.tools_alloc(input.length);
    new Uint8Array(this.exports.memory.buffer, pointer, input.length).set(input);
    const output = this.exports.tools_call(pointer, input.length);
    this.exports.tools_free(pointer, input.length);
    const address = Number(output >> 32n);
    const length = Number(output & 0xffffffffn);
    const text = new TextDecoder().decode(new Uint8Array(this.exports.memory.buffer, address, length));
    this.exports.tools_free(address, length);
    const response = parseJson<{ value: T } | { error: SdkError }>(text);
    if ('error' in response) throw new EngineError(response.error.problem, response.error.message);
    return response.value;
  }
}
