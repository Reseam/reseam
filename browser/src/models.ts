// Generated from crates/model by `cargo xtask regen browser`. Do not edit.
export type InspectRequest = {
  apk_path: string | null;
  split_paths: Array<string>;
  bundle_paths: Array<string>;
  /**
   * Marks approved signers in the response without gating catalog inspection.
   */
  trust: Trust;
};
export type InspectResponse = {
  apk: ApkMetadata | null;
  bundles: Array<BundleMetadata>;
  patches: Array<PatchMetadata>;
};
export type ApkMetadata = {
  application_label: string | null;
  package_name: string | null;
  version_name: string | null;
  version_code: number | null;
  /**
   * The container format the input came from, when it was an APKM/XAPK file.
   */
  bundle_kind: ContainerFormat | null;
  dex_files: number;
  component_count: number;
  split_names: Array<string>;
  class_count: number;
  method_count: number;
};
export type ContainerFormat = 'apkm' | 'xapk';
export type BundleMetadata = {
  file_name: string;
  name: string;
  author: string;
  description: string;
  /**
   * Empty when the manifest names none.
   */
  license: string;
  files: Array<string>;
  public_key: string;
  /**
   * The release the bundle is; empty for a development build.
   */
  version: string;
  engine: string;
  /**
   * Whether the host approves this signer for execution; inspection does not require it.
   */
  trusted: boolean;
  /**
   * Set when the signed catalog cannot be read; its patches are then absent.
   */
  problem: Problem | null;
};
export type PatchMetadata = {
  incompatibility: Incompatibility | null;
  /**
   * The presets that select this patch for the inspected APK.
   */
  presets: Array<PatchPreset>;
  /**
   * What users see. Different patches may share a display name.
   */
  name: string;
  /**
   * Hidden patches are dependencies of other patches: never listed to
   * users and never selected on their own.
   */
  hidden: boolean;
  description: string;
  enabled_by_default: boolean;
  /**
   * The patches that run first.
   */
  dependencies: Array<PatchRef>;
  compatibility: Compatibility;
  options: Array<OptionDeclaration>;
  /**
   * Replaces the APK files the engine writes, before they are signed. A mount
   * build leaves such a patch out, since it cannot tell what the files hold.
   */
  rewrites_output: boolean;
  bundle: string;
  /**
   * `<package>.<property>` of the public declaration; unique within the bundle.
   */
  id: string;
};
export type PatchSpec = {
  /**
   * What users see. Different patches may share a display name.
   */
  name: string;
  /**
   * Hidden patches are dependencies of other patches: never listed to
   * users and never selected on their own.
   */
  hidden: boolean;
  description: string;
  enabled_by_default: boolean;
  /**
   * The patches that run first.
   */
  dependencies: Array<PatchRef>;
  compatibility: Compatibility;
  options: Array<OptionDeclaration>;
  /**
   * Replaces the APK files the engine writes, before they are signed. A mount
   * build leaves such a patch out, since it cannot tell what the files hold.
   */
  rewrites_output: boolean;
  bundle: string;
  /**
   * `<package>.<property>` of the public declaration; unique within the bundle.
   */
  id: string;
};
export type PatchRef = {
  bundle: string;
  /**
   * `<package>.<property>` of the public declaration; unique within the bundle.
   */
  id: string;
};
export type PatchPreset = 'recommended' | 'all' | 'none';
export type Compatibility = { kind: 'universal' } | { kind: 'packages'; packages: Array<CompatiblePackage> };
export type CompatiblePackage = {
  package: string;
  /**
   * Accepted version names. With `version_codes` also empty, every version.
   */
  versions: Array<string>;
  /**
   * Accepted version codes; an APK matches when its name or its code is listed.
   */
  version_codes?: Array<number>;
};
export type Incompatibility = { kind: 'missing_package' } | { kind: 'package'; package: string } | {
  kind: 'version';
  expected_names: Array<string>;
  expected_codes: Array<number>;
  name: string | null;
  code: number | null;
};
export type OptionDeclaration = {
  key: string;
  title: string;
  description: string;
  option_type: OptionType;
  default_value: OptionValue | null;
  valid_values: Array<string> | null;
  required: boolean;
};
export type OptionType = 'string' | 'bool' | 'int' | 'float' | 'string_list' | 'path';
export type OptionValue =
  | { type: 'string'; value: string }
  | { type: 'bool'; value: boolean }
  | { type: 'int'; value: number | bigint }
  | { type: 'float'; value: number }
  | { type: 'string_list'; value: Array<string> }
  | { type: 'path'; value: string };
export type PatchRequest = {
  apk_path: string;
  split_paths: Array<string>;
  bundle_paths: Array<string>;
  trust: Trust;
  selection: PatchSelection;
  output: PatchOutput;
  /**
   * When absent, reuse or generate a pair next to the output.
   * Supplied paths also generate on first use when both files are missing.
   * Exactly one existing file is an error; existing credentials are never replaced.
   */
  signing: SigningKeyFiles | null;
  dry_run: boolean;
  install_method: InstallMethod;
  /**
   * Writes the output without the patches that failed, and the patches that
   * need them, instead of failing the run. The run undoes a failed patch, so
   * the output is what it would have made had the patch not been selected.
   */
  leave_out_failed: boolean;
};
export type Trust = { keys: Array<string> };
export type PatchSelection = {
  preset: PatchPreset;
  enable: Array<string>;
  disable: Array<string>;
  options: { [key in string]: { [key in string]: OptionValue } };
  /**
   * Run patches on app versions they were not declared for. The package
   * check still applies.
   */
  ignore_versions: boolean;
};
export type PatchOutput = { kind: 'auto'; path: string } | { kind: 'single_file'; path: string } | {
  kind: 'split_dir';
  path: string;
};
export type SigningKeyFiles = { key: string; cert: string };
export type InstallMethod = 'install' | 'mount';
export type PatchOutcome = { output: PatchArtifact; results: Array<PatchResult>; metrics: PatchMetrics };
export type PatchArtifact = { kind: 'single_file'; path: string } | { kind: 'split_dir'; path: string };
export type PatchResult = {
  patch: PatchRef;
  /**
   * Internal patches run as dependencies and are never listed to users.
   */
  hidden: boolean;
  /**
   * The running patches that pulled this one in, empty when it was asked
   * for directly.
   */
  required_by: Array<PatchRef>;
  status: PatchStatus;
  logs: Array<LogEntry>;
};
export type PatchStatus = { kind: 'applied' } | { kind: 'skipped'; reason: string } | {
  kind: 'unmountable';
  reason: string;
} | { kind: 'failed'; reason: string };
export type LogEntry = { level: LogLevel; patch: PatchRef; message: string };
export type LogLevel = 'DEBUG' | 'INFO' | 'WARN';
export type RunEvent =
  | { type: 'info'; message: string }
  | { type: 'patch_started'; patch: PatchRef }
  | { type: 'patch_log' } & LogEntry
  | { type: 'patch_finished'; patch: PatchRef; status: PatchStatus }
  | { type: 'patches_reverted'; patches: Array<PatchRef> };
export type PatchMetrics = {
  total_duration_ms: number;
  final_rss_bytes: number | null;
  peak_rss_bytes: number | null;
  final_rss_anon_bytes: number | null;
  final_rss_file_bytes: number | null;
  phases: Array<PatchPhaseMetrics>;
  apply_diagnostics: ApplyDiagnostics | null;
};
export type PatchPhase =
  | 'open_apk'
  | 'load_bundles'
  | 'validate_patches'
  | 'apply_patches'
  | 'write_unsigned_artifacts'
  | 'load_signing_key'
  | 'rewrite_output'
  | 'sign_artifacts';
export type PatchPhaseMetrics = {
  phase: PatchPhase;
  duration_ms: number;
  rss_bytes: number | null;
  peak_rss_bytes: number | null;
};
export type ApplyDiagnostics = { rss_bytes: number | null; dex: MemoryBreakdown; jvm: JvmHeapStats | null };
export type MemoryBreakdown = {
  raw_buffer_bytes: number;
  string_pool_bytes: number;
  string_count: number;
  id_table_bytes: number;
  class_def_bytes: number;
  /**
   * Heap held by the materialized class data, methods and instructions.
   */
  materialized_ir_bytes: number;
  materialized: MaterializationStats;
};
export type MaterializationStats = {
  total_classes: number;
  resolved_classes: number;
  methods: number;
  instructions: number;
};
export type JvmHeapStats = { used: number; committed: number; max: number };
export type Problem =
  | { type: 'bundle_too_old'; bundle: string; built: string; running: string }
  | { type: 'engine_too_old'; bundle: string; built: string; running: string }
  | { type: 'untrusted_bundle'; path: string; public_key: string }
  | { type: 'unreadable_bundle'; path: string }
  | { type: 'unreadable_apk'; path: string }
  | { type: 'patches_failed'; patches: Array<PatchRef> }
  | { type: 'single_file_components'; components: number }
  | { type: 'unknown_preset'; value: string }
  | { type: 'other' };
export type SdkError = { problem: Problem; message: string };
