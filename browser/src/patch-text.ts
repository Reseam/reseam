// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { OptionTranslation, OptionValue, PatchSpec, PatchText, PatchTranslation } from './models';

/** Patch text is written in English, so English readers stop at the source. */
const SOURCE_LANGUAGE = 'en';

/**
 * What users who read `locales`, BCP 47 tags most preferred first, see of `spec`. It resolves the
 * way the engine's `PatchSpec::text` does: each field comes from the first translation in the
 * reading order that has it, and a field none has keeps the source text.
 */
export function patchText(spec: PatchSpec, locales: readonly string[]): PatchText {
  const chain = readingOrder(spec.translations ?? [], locales);
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
      presets: (declared.presets ?? []).map((preset) => ({
        label:
          chain
            .flatMap((it) => it.options ?? [])
            .filter((it) => it.key === declared.key)
            .flatMap((it) => it.presets ?? [])
            .find((it) => sameValue(it.value, preset.value))?.label ?? preset.label,
        value: preset.value,
      })),
    })),
  };
}

function sameValue(a: OptionValue, b: OptionValue): boolean {
  return a.type === b.type && JSON.stringify(a.value) === JSON.stringify(b.value);
}

type Tag = { language: string; script?: string; region?: string };

enum Rank {
  Region,
  NoRegion,
  OtherRegion,
}

/**
 * For each locale, translations of its language in the same script, its region first, then no
 * region, then other regions. An English locale ends the order after only its own region.
 */
function readingOrder(translations: readonly PatchTranslation[], locales: readonly string[]): PatchTranslation[] {
  const available = translations.flatMap((it) => {
    const tag = parse(it.locale);
    return tag ? [{ tag, it }] : [];
  });
  const order: PatchTranslation[] = [];
  for (const locale of locales) {
    const wanted = parse(locale);
    if (!wanted) continue;
    const source = wanted.language === SOURCE_LANGUAGE;
    available
      .flatMap(({ tag, it }) => {
        const fit = rank(tag, wanted);
        return fit === undefined || (source && fit !== Rank.Region) ? [] : [{ fit, it }];
      })
      .sort((a, b) => a.fit - b.fit)
      .forEach(({ it }) => order.includes(it) || order.push(it));
    if (source) break;
  }
  return order;
}

function rank(tag: Tag, wanted: Tag): Rank | undefined {
  if (tag.language !== wanted.language || tag.script !== wanted.script) return undefined;
  if (tag.region === undefined) return Rank.NoRegion;
  return tag.region === wanted.region ? Rank.Region : Rank.OtherRegion;
}

/** Canonical language, likely script and the region the tag names, or nothing for a malformed tag. */
function parse(tag: string): Tag | undefined {
  let locale: Intl.Locale;
  try {
    locale = new Intl.Locale(Intl.getCanonicalLocales(tag)[0]!);
  } catch {
    return undefined;
  }
  return { language: locale.language, script: locale.maximize().script, region: locale.region };
}
