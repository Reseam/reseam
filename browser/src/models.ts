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
  /**
   * The container format the input came from, when it was an APKM, XAPK or
   * APKS file.
   */
  bundle_kind: ContainerFormat | null;
  dex_files: number;
  component_count: number;
  split_names: Array<string>;
  class_count: number;
  method_count: number;
  /**
   * Absent when the manifest names no package; only universal patches
   * apply to such an app.
   */
  package_name: string | null;
  version_name: string | null;
  version_code: number | null;
};
export type ContainerFormat = 'apkm' | 'xapk' | 'apks';
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
   * Hex hash of the bundle files the patch's module and the modules it uses are
   * built into. A patch keeps it across bundle releases that rebuild none of them.
   */
  fingerprint: string;
  bundle: string;
  /**
   * `<package>.<property>` of the public declaration; unique within the bundle.
   */
  id: string;
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
};
export type PatchSpec = {
  bundle: string;
  /**
   * `<package>.<property>` of the public declaration; unique within the bundle.
   */
  id: string;
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
};
export type PatchRef = string;
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
export type OptionProblem =
  | { kind: 'missing' }
  | { kind: 'undeclared' }
  | { kind: 'wrong_type'; expected: OptionType; actual: OptionType }
  | { kind: 'not_a_bool' }
  | { kind: 'not_an_integer' }
  | { kind: 'not_a_number' }
  | { kind: 'not_allowed'; value: string; allowed: Array<string> };
export type ParsedOption = { kind: 'value'; value: OptionValue } | { kind: 'problem'; problem: OptionProblem };
export type PlanRequest = {
  /**
   * The patches an inspection lists.
   */
  patches: Array<PatchMetadata>;
  app: AppIdentity;
  selection: PatchSelection;
  install_method: InstallMethod;
};
export type AppIdentity = {
  /**
   * Absent when the manifest names no package; only universal patches
   * apply to such an app.
   */
  package_name: string | null;
  version_name: string | null;
  version_code: number | null;
};
export type SelectionPlan = {
  /**
   * The patches users see for the app, those that declare its package
   * and universal ones, then the hidden patches the run needs, in request
   * order.
   */
  patches: Array<PlannedPatch>;
  /**
   * Selection entries that name no patch, such as patches a later bundle
   * release removed. The plan leaves them out; a run fails on them.
   */
  unknown: Array<string>;
};
export type PlannedPatch = {
  patch: PatchRef;
  /**
   * Why the patch does not apply to the app.
   */
  incompatibility: Incompatibility | null;
  /**
   * The presets that select the patch for the app.
   */
  presets: Array<PatchPreset>;
  /**
   * The selection asks for the patch, as opposed to a patch needing it.
   */
  selected: boolean;
  run: PlannedRun;
  /**
   * The selected patches that need this one, empty when it is selected.
   */
  required_by: Array<PatchRef>;
  /**
   * What is wrong with the option values the patch would run with.
   */
  option_problems: Array<OptionIssue>;
};
export type PlannedRun = { kind: 'off' } | { kind: 'runs' } | { kind: 'skipped'; reason: string } | {
  kind: 'unmountable';
  reason: string;
};
export type OptionIssue = { key: string; problem: OptionProblem };
export type BuildRecord = {
  /**
   * The app the build patched.
   */
  app: AppIdentity;
  /**
   * The selection as the request gave it.
   */
  selection: PatchSelection;
  install_method: InstallMethod;
  /**
   * Every patch the run took up, applied or not, dependencies included,
   * so a fix to one that failed counts as a change.
   */
  patches: Array<RecordedPatch>;
};
export type RecordedPatch = {
  patch: PatchRef;
  /**
   * The patch's fingerprint when the build ran it.
   */
  fingerprint: string;
};
export type ChangesRequest = {
  record: BuildRecord;
  /**
   * The patches an inspection lists now.
   */
  patches: Array<PatchMetadata>;
};
export type PatchChange = { kind: 'changed'; patch: PatchRef } | { kind: 'added'; patch: PatchRef } | {
  kind: 'removed';
  patch: PatchRef;
};
export type ReleaseIndex = {
  /**
   * Who publishes the releases. The key is `bundle` in every index, as the
   * patches index it was first written for named it.
   */
  bundle: Publisher;
  /**
   * Newest first.
   */
  releases: Array<PublishedRelease>;
};
export type Publisher = {
  name: string;
  author: string;
  description: string;
  homepage?: string | null;
  /**
   * The bundle signer in hexadecimal; absent from `manager.json`.
   */
  public_key?: string | null;
};
export type PublishedRelease = {
  version: string;
  /**
   * RFC 3339.
   */
  created_at: string;
  /**
   * The release notes, in Markdown.
   */
  description: string;
  download_url: string;
  prerelease: boolean;
  /**
   * The bundle's patches. Only the newest stable release and the newest
   * prerelease keep their list, so older ones and `manager.json` have none.
   */
  patches?: Array<PatchSpec> | null;
};
export type ReleaseChannel = 'stable' | 'prerelease';
export type PatchRequest = {
  apk_path: string;
  split_paths: Array<string>;
  bundle_paths: Array<string>;
  trust: Trust;
  selection: PatchSelection;
  output: PatchOutput;
  /**
   * When absent, reuse or generate a key pair next to the output.
   */
  signing: SigningIdentity | null;
  dry_run: boolean;
  install_method: InstallMethod;
  /**
   * Writes the output without the patches that failed, and the patches that
   * need them, instead of failing the run. The run undoes a failed patch, so
   * the output is what it would have made had the patch not been selected.
   */
  leave_out_failed: boolean;
  /**
   * Logs each place a patch changed app code when the app runs it, under the
   * tag `reseam-trace`, so a test build shows which edits ran on the device.
   */
  trace: boolean;
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
} | { kind: 'split_archive'; path: string };
export type SigningIdentity = { kind: 'files'; key: string; cert: string } | {
  kind: 'pkcs12';
  path: string;
  password: string;
};
export type SigningKeyMaterial = {
  /**
   * The private key in PKCS#8 DER.
   */
  key: Array<number>;
  /**
   * The X.509 certificate in DER.
   */
  cert: Array<number>;
};
export type InstalledApp = {
  /**
   * The long version code the package manager reports.
   */
  version_code: number;
  version_name: string;
  /**
   * The base APK's file name in the app's folder.
   */
  base_file: string;
  /**
   * The splits it is installed with, by name, and the file holding each.
   */
  splits: Array<InstalledSplit>;
  /**
   * The app's `.dm` file, when it has one. ART reads it before any other
   * profile, so a mount has to cover it.
   */
  dex_metadata_file: string | null;
};
export type InstalledSplit = {
  name: string;
  /**
   * The file name in the app's folder.
   */
  file: string;
};
export type MountRequest = {
  package_name: string;
  /**
   * The app's name, for the root manager's module list.
   */
  label: string;
  /**
   * The patched build's version code, which has to be the installed one.
   */
  version_code: number;
  installed: InstalledApp;
  /**
   * The patched base APK, at its path on the device.
   */
  base: string;
  splits: Array<PatchedSplit>;
  /**
   * The patched base APK's dex metadata at its path on the device; give it
   * when the installed app has a `.dm`.
   */
  dex_metadata: string | null;
};
export type PatchedSplit = { name: string; path: string };
export type RootScript = { script: string; namespace: MountNamespace };
export type MountNamespace = 'global' | 'private';
export type InstallMethod = 'install' | 'mount';
export type PatchOutcome = {
  output: PatchArtifact;
  results: Array<PatchResult>;
  /**
   * What the build was made from; keep it to learn later whether patching
   * again would change the build.
   */
  record: BuildRecord;
  metrics: PatchMetrics;
};
export type PatchArtifact = { kind: 'single_file'; path: string } | { kind: 'split_dir'; path: string } | {
  kind: 'split_archive';
  path: string;
};
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
  | { type: 'phase_started'; phase: PatchPhase }
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
  | { type: 'installed_app_differs'; package: string; installed: string }
  | { type: 'app_not_installed'; package: string }
  | { type: 'keystore_locked' }
  | { type: 'keystore_empty' }
  | { type: 'unsupported_signing_key' }
  | { type: 'unknown_preset'; value: string }
  | { type: 'other' };
export type SdkError = { problem: Problem; message: string };
