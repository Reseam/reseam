// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { EngineError, EngineTools, runtime } from '../src';
import type { OptionDeclaration, PatchMetadata } from '../src';

const tools = await EngineTools.load(pathToFileURL(`${import.meta.dir}/../public/${runtime.base}`).href);

const option: OptionDeclaration = {
  key: 'count', title: 'Count', description: '', option_type: 'int', default_value: null, valid_values: null, required: true,
};
const patch: PatchMetadata = {
  bundle: 'bundle', id: 'patch', name: 'Patch', hidden: false, description: '', enabled_by_default: true,
  dependencies: [], compatibility: { kind: 'universal' }, options: [option], rewrites_output: false,
  incompatibility: null, presets: [], fingerprint: '',
};

test('plans a selection and reports its missing options', () => {
  const plan = tools.plan({
    patches: [patch],
    app: { package_name: 'com.example', version_name: null, version_code: null },
    selection: { preset: 'recommended', enable: ['bundle/removed'], disable: [], options: {}, ignore_versions: false },
    install_method: 'install',
  });
  expect(plan.unknown).toEqual(['bundle/removed']);
  expect(plan.patches[0].run).toEqual({ kind: 'runs' });
  expect(plan.patches[0].option_problems).toEqual([{ key: 'count', problem: { kind: 'missing' } }]);
});

test('reads option text as the engine does, longs included', () => {
  expect(tools.parseOption(option, ' 9007199254740993 ')).toEqual({ kind: 'value', value: { type: 'int', value: 9007199254740993n } });
  expect(tools.parseOption(option, '1.5')).toEqual({ kind: 'problem', problem: { kind: 'not_an_integer' } });
  expect(tools.optionText({ type: 'string_list', value: ['a', 'b'] })).toBe('a\nb');
});

test('backs a key up as PKCS#12 and reads it back', () => {
  const keystore = readFileSync(`${import.meta.dir}/../../crates/sign/tests/keystores/pkcs12-legacy.p12`);
  const key = tools.importPkcs12(keystore, 'reseam-test');
  const restored = tools.importPkcs12(tools.exportPkcs12(key, 'backup'), 'backup');
  expect(restored).toEqual(key);
  expect(tools.certificateFingerprint(Uint8Array.from(key.cert))).toMatch(/^([0-9A-F]{2}:){31}[0-9A-F]{2}$/);
  expect(() => tools.importPkcs12(keystore, 'wrong')).toThrow(EngineError);
});
