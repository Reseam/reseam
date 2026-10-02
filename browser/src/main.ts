// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
import './style.css';
import { BrowserSession, type MountedFile, type Artifact } from './session';
import { preference, savePreference, type Identity } from './preferences';
import { stringifyJson } from './json';
import type { Inspection, OptionDeclaration, OptionValue, Outcome, PatchMetadata, Preset } from './models';

if (import.meta.env.VITE_TEST_HARNESS === '1') Object.assign(window, { BrowserSession });
const element = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const picked = (id: string) => [...(element<HTMLInputElement>(id).files ?? [])];
const status = (message: string) => { element('status').textContent = message; };
const urls: string[] = [];
const choices = new Map<string, HTMLInputElement>();
const approvals = new Map<string, HTMLInputElement>();
interface OptionInput { declaration: OptionDeclaration; input: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement; folder?: HTMLInputElement; path: string }
const inputs = new Map<string, OptionInput[]>();
let inspection: Inspection | undefined;
let files: MountedFile[] = [];
let apkPath: string;
let splitPaths: string[] = [];
let bundlePaths: string[] = [];
let session: BrowserSession | undefined;
let controller = new AbortController();
let busy = false;
let active = new Set<string>();
function log(message: string): void {
  const output = element('log');
  output.textContent = ((output.textContent ?? '') + message + '\n').slice(-100_000);
  output.scrollTop = output.scrollHeight;
}
function showError(error: unknown): void {
  element('error').hidden = false;
  element('error').textContent = error instanceof Error ? error.message : String(error);
  log(String(error));
}
function working(value: boolean): void {
  busy = value;
  element<HTMLButtonElement>('inspect').disabled = value;
  element('cancel').hidden = !value;
  for (const id of ['apk', 'splits', 'bundles', 'preset', 'ignore-versions', 'key', 'cert']) element<HTMLInputElement>(id).disabled = value;
  for (const input of approvals.values()) input.disabled = value;
  updateSelection();
}
async function clearSession(): Promise<void> {
  for (const url of urls) URL.revokeObjectURL(url);
  urls.length = 0; element('downloads').replaceChildren();
  const previous = session; session = undefined;
  if (previous) await previous.dispose();
}
function paragraph(text: string): HTMLParagraphElement {
  const result = document.createElement('p'); result.textContent = text; return result;
}
function label(text: string, input: HTMLElement): HTMLLabelElement {
  const result = document.createElement('label'); result.append(input, document.createTextNode(' ' + text)); return result;
}
function reference(patch: PatchMetadata): string { return `${patch.bundle}/${patch.id}`; }
function compatible(patch: PatchMetadata): boolean {
  const packageMatches = patch.compatibility.kind === 'universal' || patch.compatibility.packages.some(pkg => pkg.package === inspection?.apk?.package_name);
  return packageMatches && (!patch.incompatibility || element<HTMLInputElement>('ignore-versions').checked);
}
function updateSelection(): void {
  for (const patch of inspection?.patches ?? []) {
    const choice = choices.get(reference(patch));
    if (choice && !compatible(patch)) choice.checked = false;
  }
  active = new Set([...choices].filter(([, input]) => input.checked).map(([key]) => key));
  const include = (key: string) => {
    const patch = inspection?.patches.find(patch => reference(patch) === key);
    for (const dependency of patch?.dependencies ?? []) if (!active.has(dependency)) { active.add(dependency); include(dependency); }
  };
  for (const key of [...active]) include(key);
  for (const patch of inspection?.patches ?? []) {
    const key = reference(patch), choice = choices.get(key);
    if (choice) { choice.disabled = busy || !compatible(patch); if (!compatible(patch)) choice.checked = false; }
    for (const option of inputs.get(key) ?? []) {
      option.input.disabled = busy || !active.has(key);
      option.input.required = active.has(key) && option.declaration.required && option.declaration.default_value === null && !option.folder?.files?.length;
      if (option.folder) option.folder.disabled = option.input.disabled;
    }
  }
  element<HTMLButtonElement>('patch').disabled = busy || !inspection || inspection.bundles.some(bundle => bundle.problem || !approvals.get(bundle.public_key)?.checked);
}
function optionInput(option: OptionDeclaration, index: number): OptionInput {
  const path = `options/${index}`;
  const input = option.option_type === 'string_list' ? document.createElement('textarea') :
    option.option_type === 'bool' || (option.valid_values && option.option_type === 'string') ? document.createElement('select') : document.createElement('input');
  if (input instanceof HTMLSelectElement) {
    input.append(new Option('Use default', ''));
    for (const value of option.option_type === 'bool' ? ['true', 'false'] : option.valid_values!) input.append(new Option(value, value));
  } else if (input instanceof HTMLInputElement) {
    input.type = option.option_type === 'bool' ? 'checkbox' : option.option_type === 'path' ? 'file' : 'text';
  }
  const value = option.default_value?.value;
  if (input instanceof HTMLInputElement && input.type === 'checkbox') input.checked = value === true;
  else if (input instanceof HTMLInputElement && input.type === 'file') { /* File inputs cannot carry defaults. */ }
  else input.value = Array.isArray(value) ? value.join('\n') : value === undefined ? '' : String(value);
  if (input instanceof HTMLInputElement && ['int', 'float'].includes(option.option_type)) input.inputMode = option.option_type === 'int' ? 'numeric' : 'decimal';
  const folder = option.option_type === 'path' ? document.createElement('input') : undefined;
  if (folder) { folder.type = 'file'; folder.multiple = true; folder.setAttribute('webkitdirectory', ''); folder.onchange = () => { input.removeAttribute('required'); updateSelection(); }; }
  return { declaration: option, input, folder, path };
}
async function render(value: Inspection): Promise<void> {
  choices.clear(); approvals.clear(); inputs.clear();
  for (const id of ['patches', 'trust', 'options']) element(id).replaceChildren();
  element('app').textContent = `${value.apk?.application_label ?? value.apk?.package_name ?? 'App'} · ${value.apk?.version_name ?? 'unknown version'} · ${value.apk?.component_count ?? 0} APK component(s)`;
  const trusted = await preference<string[]>('trusted-signers') ?? [];
  for (const bundle of value.bundles) {
    if (approvals.has(bundle.public_key)) { element('trust').append(paragraph(`${bundle.name} shares the signer above`)); continue; }
    const box = document.createElement('input'); box.type = 'checkbox'; box.checked = trusted.includes(bundle.public_key); box.onchange = updateSelection;
    if (bundle.problem) { element('trust').append(paragraph(`${bundle.file_name}: ${stringifyJson(bundle.problem)}`)); continue; }
    approvals.set(bundle.public_key, box);
    const identity = document.createElement('code'); identity.textContent = bundle.public_key;
    element('trust').append(label(`${bundle.name} by ${bundle.author || 'unknown author'}`, box), identity);
  }
  let index = 0;
  for (const patch of value.patches) {
    const key = reference(patch);
    if (!patch.hidden) {
      const box = document.createElement('input'); box.type = 'checkbox'; box.checked = patch.presets.includes('recommended'); box.onchange = updateSelection;
      choices.set(key, box);
      element('patches').append(label(patch.name, box));
      if (patch.incompatibility) { const detail = document.createElement('small'); detail.textContent = patch.incompatibility; element('patches').append(detail); }
    }
    if (patch.options.length) {
      const group = document.createElement('fieldset'); const legend = document.createElement('legend'); legend.textContent = patch.name + ' options'; group.append(legend);
      const values: OptionInput[] = [];
      for (const option of patch.options) {
        const field = optionInput(option, index++); values.push(field);
        group.append(label(option.title + (option.required ? ' (required)' : ''), field.input));
        if (field.folder) group.append(label('Or choose a folder', field.folder));
        if (option.description) { const description = document.createElement('small'); description.textContent = option.description; group.append(description); }
      }
      inputs.set(key, values); element('options').append(group);
    }
  }
  element<HTMLSelectElement>('preset').value = 'recommended';
  element('configuration').hidden = false; updateSelection();
}
function resolveOptions(): { options: Record<string, Record<string, OptionValue>>; uploads: MountedFile[] } {
  const options: Record<string, Record<string, OptionValue>> = {}, uploads: MountedFile[] = [];
  for (const [key, values] of inputs) {
    if (!active.has(key)) continue;
    for (const { declaration, input, folder, path } of values) {
      let value: OptionValue['value'];
      if (declaration.option_type === 'path') {
        const selected = (input as HTMLInputElement).files?.[0];
        const directory = [...(folder?.files ?? [])];
        if (selected && directory.length) throw new Error(`${declaration.title}: choose a file or a folder`);
        if (selected) { const name = `${path}/file`; uploads.push({ name, file: selected }); value = '/input/' + name; }
        else if (directory.length) {
          for (const file of directory) uploads.push({ name: `${path}/${file.webkitRelativePath.split('/').slice(1).join('/')}`, file });
          value = '/input/' + path;
        } else continue;
      } else if (declaration.option_type === 'bool') { if (!input.value) continue; value = input.value === 'true'; }
      else if (input.value === '' && declaration.default_value === null && !declaration.required) continue;
      else if (declaration.option_type === 'int') {
        if (!/^-?\d+$/.test(input.value)) throw new Error(`${declaration.title}: enter an integer`);
        value = BigInt(input.value);
        if (value < -9223372036854775808n || value > 9223372036854775807n) throw new Error(`${declaration.title}: integer is outside the supported range`);
      } else if (declaration.option_type === 'float') {
        value = Number(input.value); if (!input.value.trim() || !Number.isFinite(value)) throw new Error(`${declaration.title}: enter a finite number`);
      } else if (declaration.option_type === 'string_list') value = input.value.split('\n').filter(Boolean);
      else if (input instanceof HTMLSelectElement && !input.value) continue;
      else value = input.value;
      (options[key] ??= {})[declaration.key] = { type: declaration.option_type, value };
    }
  }
  return { options, uploads };
}
function download(artifact: Artifact): void {
  const url = URL.createObjectURL(artifact.file); urls.push(url);
  const link = document.createElement('a'); link.href = url; link.download = artifact.name.split('/').at(-1)!;
  link.textContent = `${artifact.name} (${(artifact.file.size / 1024 / 1024).toFixed(2)} MiB)`; element('downloads').append(link);
}
const runtimeOptions = () => ({
  signal: controller.signal, licenseKey: import.meta.env.VITE_CHEERPJ_LICENSE_KEY,
  onLog: (message: string) => { log(message); status(message); },
  onEvent: (event: unknown) => { log(stringifyJson(event)); const info = event as { type: string; message?: string; patch?: string }; if (info.message || info.patch) status(info.message ?? info.patch!); },
});
element<HTMLFormElement>('inputs').onsubmit = async event => {
  event.preventDefault(); if (busy) return;
  controller = new AbortController(); element('error').hidden = true; working(true); status('Inspecting files…');
  try {
    await clearSession(); element('log').textContent = ''; inspection = undefined; element('configuration').hidden = true;
    const apk = picked('apk')[0]; const extension = apk.name.split('.').at(-1)!.toLowerCase();
    apkPath = `/input/app.${extension}`; files = [{ name: `app.${extension}`, file: apk }];
    splitPaths = picked('splits').map((file, i) => { const name = `split-${i}.apk`; files.push({ name, file }); return '/input/' + name; });
    bundlePaths = picked('bundles').map((file, i) => { const name = `bundle-${i}.reseam`; files.push({ name, file }); return '/input/' + name; });
    session = await BrowserSession.open(files, runtimeOptions());
    inspection = await session.request<Inspection>('inspect', { apk_path: apkPath, split_paths: splitPaths, bundle_paths: bundlePaths });
    await render(inspection); status('Choose patches and approve their signers');
  } catch (error) { showError(error); status(controller.signal.aborted ? 'Cancelled' : 'Inspection failed'); await clearSession().catch(showError); }
  finally { working(false); }
};
element<HTMLFormElement>('configuration').onsubmit = async event => {
  event.preventDefault(); if (busy || !inspection) return;
  element('error').hidden = true;
  try {
    const selection = resolveOptions(); const importedKey = picked('key')[0], importedCert = picked('cert')[0];
    if (!!importedKey !== !!importedCert) throw new Error('Import both the private key and its certificate');
    if ((importedKey?.size ?? 0) > 1_048_576 || (importedCert?.size ?? 0) > 1_048_576) throw new Error('Signing identity files are too large');
    controller = new AbortController(); working(true); status('Preparing patch run…');
    const keys = [...approvals].filter(([, input]) => input.checked).map(([key]) => key);
    const trusted = new Set(await preference<string[]>('trusted-signers') ?? []);
    for (const [key, input] of approvals) input.checked ? trusted.add(key) : trusted.delete(key);
    await savePreference('trusted-signers', [...trusted]);
    await clearSession();
    await navigator.locks.request('reseam-signing-identity', { signal: controller.signal }, async () => {
      let identity = importedKey && importedCert ? { key: await importedKey.arrayBuffer(), cert: await importedCert.arrayBuffer() } : await preference<Identity>('identity');
      const credentials: MountedFile[] = identity ? [
        { name: 'reseam.pk8', file: new File([identity.key], 'reseam.pk8'), directory: 'identity' },
        { name: 'reseam.der', file: new File([identity.cert], 'reseam.der'), directory: 'identity' },
      ] : [];
      session = await BrowserSession.open([...files, ...selection.uploads, ...credentials], runtimeOptions());
      await session.request<Outcome>('patch', {
        apk_path: apkPath, split_paths: splitPaths, bundle_paths: bundlePaths, trust: { keys },
        selection: { preset: 'none', enable: [...choices].filter(([, input]) => input.checked).map(([key]) => key), options: selection.options, ignore_versions: element<HTMLInputElement>('ignore-versions').checked },
        output: { kind: 'auto', path: '/output/patched' }, signing: { key: '/identity/reseam.pk8', cert: '/identity/reseam.der' },
      });
      const artifacts = await session.artifacts();
      if (!identity) {
        const key = artifacts.find(file => file.name === 'identity/reseam.pk8')?.file;
        const cert = artifacts.find(file => file.name === 'identity/reseam.der')?.file;
        if (!key || !cert) throw new Error('Signing identity is missing from the completed run');
        identity = { key: await key.arrayBuffer(), cert: await cert.arrayBuffer() };
      }
      await savePreference('identity', identity);
      element('downloads').append(paragraph('Patch complete. Download the APK(s) and back up the signing identity.'));
      for (const artifact of artifacts.filter(file => !file.name.startsWith('identity/'))) download(artifact);
      download({ name: 'reseam.pk8', file: new File([identity.key], 'reseam.pk8') });
      download({ name: 'reseam.der', file: new File([identity.cert], 'reseam.der') });
      status('Patch complete');
    });
  } catch (error) { showError(error); status(controller.signal.aborted ? 'Cancelled' : 'Patching failed'); await clearSession().catch(showError); }
  finally { working(false); }
};
element<HTMLSelectElement>('preset').onchange = () => {
  const preset = element<HTMLSelectElement>('preset').value as Preset;
  for (const patch of inspection?.patches ?? []) { const box = choices.get(reference(patch)); if (box) box.checked = compatible(patch) && patch.presets.includes(preset); }
  updateSelection();
};
element<HTMLInputElement>('ignore-versions').onchange = updateSelection;
element('cancel').onclick = () => { controller.abort(); status('Cancelling…'); };
for (const id of ['apk', 'splits', 'bundles']) element(id).onchange = () => { inspection = undefined; element('configuration').hidden = true; updateSelection(); };
addEventListener('pagehide', () => { void session?.dispose().catch(error => log(String(error))); });
if (!crossOriginIsolated) { showError(new Error('This page needs cross-origin isolation headers before browser patching can run.')); element<HTMLButtonElement>('inspect').disabled = true; }
