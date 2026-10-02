// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
export type Preset = 'recommended' | 'all' | 'none';
export type OptionKind = 'string' | 'bool' | 'int' | 'float' | 'string_list' | 'path';
export interface OptionValue { type: OptionKind; value: string | boolean | number | bigint | string[] }
export interface OptionDeclaration {
  key: string; title: string; description: string; option_type: OptionKind;
  default_value: OptionValue | null; valid_values: string[] | null; required: boolean;
}
export interface PatchMetadata {
  bundle: string; id: string; name: string; hidden: boolean; description: string;
  dependencies: string[]; options: OptionDeclaration[]; incompatibility: string | null; presets: Preset[];
  compatibility: { kind: 'universal' } | { kind: 'packages'; packages: { package: string; versions: string[] }[] };
}
export interface BundleMetadata {
  name: string; file_name: string; author: string; description: string; public_key: string;
  problem: { type: string; built?: string; running?: string } | null;
}
export interface Inspection {
  apk: { package_name: string | null; version_name: string | null; application_label: string | null; component_count: number } | null;
  bundles: BundleMetadata[]; patches: PatchMetadata[];
}
export interface Outcome { output: { kind: 'single_file' | 'split_dir'; path: string }; metrics: { total_duration_ms: number }; results: unknown[] }
