import { expect, test } from 'bun:test';
import { Channel, decode, encode } from '../src/channel';

test('worker bridge preserves binary data and 64-bit values across repeated requests and chunk boundaries', async () => {
  const channel = new Channel();
  const worker = new Worker(new URL('./channel-worker.ts', import.meta.url));
  const completed = new Promise((resolve, reject) => { worker.onmessage = resolve; worker.onerror = reject; });
  const sizes = [0, 1, (1 << 20) - 7, (1 << 20) + 17, 3 << 20, ...Array(200).fill(53)];
  worker.postMessage({ buffer: channel.buffer, count: sizes.length });
  try {
    for (let i = 0; i < sizes.length; i++) {
      const bytes = Uint8Array.from({ length: sizes[i] }, (_, j) => (j + i) % 251);
      const sequence = BigInt(i) + 9_223_372_036_854_770_000n;
      await channel.send(1, encode({ bytes, sequence }));
      const response = await channel.receive();
      expect(response.kind).toBe(2);
      const value = decode<{ bytes: Uint8Array; sequence: bigint }>(response.bytes);
      expect(value.sequence).toBe(sequence);
      expect(value.bytes).toEqual(bytes);
    }
    await completed;
  } finally { channel.close(); worker.terminate(); }
}, 30_000);

test('bridge rejects corrupted binary envelopes', () => {
  const message = encode({ data: new Uint8Array([1, 2, 3]) });
  for (const bytes of [message.subarray(0, 7), message.subarray(0, message.length - 1), new Uint8Array([...message, 0])]) {
    expect(() => decode(bytes)).toThrow();
  }
});
