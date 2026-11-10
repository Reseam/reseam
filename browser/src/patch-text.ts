// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { OptionTranslation, PatchSpec, PatchText, PatchTranslation } from './models';

/**
 * What users who read `locales`, BCP 47 tags most preferred first, see of `spec`. It resolves the
 * way the engine's `PatchSpec::text` does: each field comes from the first tag that translates it,
 * trying every tag as itself, then its language and script, then its language, and a field no tag
 * translates keeps the source text.
 */
export function patchText(spec: PatchSpec, locales: readonly string[]): PatchText {
  const chain = fallbacks(locales).flatMap((tag) => spec.translations?.filter((it) => it.locale === tag) ?? []);
  const pick = (source: string, field: (it: PatchTranslation) => string | null | undefined) =>
    chain.map(field).find((it) => it != null) ?? source;
  const option = (key: string, field: (it: OptionTranslation) => string | null | undefined) =>
    chain.flatMap((it) => it.options ?? []).filter((it) => it.key === key).map(field).find((it) => it != null);
  return {
    name: pick(spec.name, (it) => it.name),
    description: pick(spec.description, (it) => it.description),
    options: spec.options.map((declared) => ({
      key: declared.key,
      title: option(declared.key, (it) => it.title) ?? declared.title,
      description: option(declared.key, (it) => it.description) ?? declared.description,
    })),
  };
}

function fallbacks(locales: readonly string[]): string[] {
  const tags: string[] = [];
  for (const locale of locales) {
    const [language = '', second] = locale.split('-');
    const script = second && /^[A-Za-z]{4}$/.test(second) ? `${language}-${second}` : null;
    for (const tag of [locale, script, language]) {
      if (tag && !tags.includes(tag)) tags.push(tag);
    }
  }
  return tags;
}
