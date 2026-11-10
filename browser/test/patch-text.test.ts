import { expect, test } from 'bun:test';
import cases from '../../crates/model/tests/data/patch_text.json';
import type { PatchSpec } from '../src/models';
import { patchText } from '../src/patch-text';

test('each field comes from the first tag that translates it, as the engine resolves it', () => {
  for (const { locales, name, title, description } of cases.cases) {
    const text = patchText(cases.spec as PatchSpec, locales);
    expect([text.name, text.options[0]?.title, text.description], locales.join(',')).toEqual([name, title, description]);
  }
});
