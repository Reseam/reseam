// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
export type Preset = 'recommended' | 'all' | 'none';
export type OptionKind = 'string' | 'bool' | 'int' | 'float' | 'string_list' | 'path';
export interface OptionValue { type: OptionKind; value: string | boolean | number | bigint | string[] }
export interface OptionDeclaration {
  key: string; title: string; description: string; option_type: OptionKind;
  default_value: OptionValue | null; valid_values: string[] | null; required: boolean;
}
export type Compatibility = { kind: 'universal' } | { kind: 'packages'; packages: { package: string; versions: string[] }[] };
export interface PatchMetadata {
  bundle: string; id: string; name: string; hidden: boolean; description: string; enabled_by_default: boolean;
  dependencies: string[]; options: OptionDeclaration[]; incompatibility: string | null; presets: Preset[];
  compatibility: Compatibility;
}
export type Problem =
  | { type: 'bundle_too_old' | 'engine_too_old'; bundle: string; built: string; running: string }
  | { type: 'untrusted_bundle'; path: string; public_key: string }
  | { type: 'unreadable_bundle' | 'unreadable_apk'; path: string }
  | { type: 'patches_failed'; patches: string[] }
  | { type: 'option_type'; key: string; expected: OptionKind; actual: OptionKind }
  | { type: 'option_choice'; key: string; value: string; allowed: string[] }
  | { type: 'single_file_components'; components: number }
  | { type: 'incompatible_package'; package: string }
  | { type: 'unknown_preset'; value: string }
  | { type: 'missing_package' | 'other' };
export interface BundleMetadata {
  file_name: string; name: string; author: string; description: string; files: string[];
  public_key: string; engine: string; trusted: boolean; problem: Problem | null;
}
export interface ApkMetadata {
  application_label: string | null; package_name: string | null; version_name: string | null; version_code: number | null;
  bundle_kind: 'apkm' | 'xapk' | null; dex_files: number; component_count: number; split_names: string[];
  class_count: number; method_count: number;
}
export interface Inspection { apk: ApkMetadata | null; bundles: BundleMetadata[]; patches: PatchMetadata[] }
export interface InspectRequest { apk_path?: string; split_paths?: string[]; bundle_paths: string[]; trust?: { keys: string[] } }
export interface PatchSelection {
  preset: Preset; enable: string[]; disable?: string[];
  options: Record<string, Record<string, OptionValue>>; ignore_versions: boolean;
}
export interface PatchRequest {
  apk_path: string; split_paths: string[]; bundle_paths: string[]; trust: { keys: string[] };
  selection: PatchSelection; output: { kind: 'auto' | 'single_file' | 'split_dir'; path: string };
  signing?: { key: string; cert: string };
}
export type PatchStatus = { kind: 'applied' } | { kind: 'skipped' | 'failed'; reason: string };
export interface LogEntry { level: 'DEBUG' | 'INFO' | 'WARN'; patch: string; message: string }
export interface PatchResult { patch: string; hidden: boolean; required_by: string[]; status: PatchStatus; logs: LogEntry[] }
export type RunEvent =
  | { type: 'info'; message: string }
  | { type: 'patch_started'; patch: string }
  | ({ type: 'patch_log' } & LogEntry)
  | { type: 'patch_finished'; patch: string; status: PatchStatus };
export type PatchPhase =
  | 'open_apk' | 'load_bundles' | 'validate_patches' | 'apply_patches'
  | 'write_unsigned_artifacts' | 'load_signing_key' | 'sign_artifacts';
export interface Outcome {
  output: { kind: 'single_file' | 'split_dir'; path: string };
  results: PatchResult[];
  metrics: { total_duration_ms: number; phases: { phase: PatchPhase; duration_ms: number }[] };
}
