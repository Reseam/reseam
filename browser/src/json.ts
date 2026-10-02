// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
import { parse, stringify } from 'lossless-json';

// Java long options and native handles must survive JSON without losing bits.
export function parseJson<T = unknown>(text: string): T {
  return parse(text, null, token => {
    const number = Number(token);
    return /^-?\d+$/.test(token) && !Number.isSafeInteger(number) ? BigInt(token) : number;
  }) as T;
}
export function stringifyJson(value: unknown): string {
  const result = stringify(value);
  if (result === undefined) throw new Error('Cannot serialize an empty host message');
  return result;
}
