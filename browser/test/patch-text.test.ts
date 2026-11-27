import { expect, test } from 'bun:test';
import cases from '../../crates/model/tests/data/patch_text.json';
import type { PatchSpec } from '../src/models';
import { patchText } from '../src/patch-text';

test('each field comes from the first tag that translates it, as the engine resolves it', () => {
  for (const { locales, name, title, description, preset } of cases.cases) {
    const text = patchText(cases.spec as PatchSpec, locales);
    const option = text.options[0];
    expect([text.name, option?.title, text.description, option?.presets[0]?.label], locales.join(',')).toEqual([
      name,
      title,
      description,
      preset,
    ]);
  }
});
