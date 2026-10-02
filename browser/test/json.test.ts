import { expect, test } from 'bun:test';
import { parseJson, stringifyJson } from '../src/json';

test('patch options retain signed 64-bit integers across the host JSON boundary', () => {
  const text = '{"options":{"small":123,"min":-9223372036854775808,"max":9223372036854775807,"float":1.25}}';
  const value = parseJson<{ options: Record<string, number | bigint> }>(text);
  expect(value.options.min).toBe(-9223372036854775808n);
  expect(value.options.max).toBe(9223372036854775807n);
  expect(value.options.small).toBe(123);
  expect(value.options.float).toBe(1.25);
  expect(parseJson<typeof value>(stringifyJson(structuredClone(value)))).toEqual(value);
});
