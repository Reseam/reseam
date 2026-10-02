import { Channel, decode, encode } from '../src/channel';
self.onmessage = event => {
  const channel = new Channel(event.data.buffer);
  for (let i = 0; i < event.data.count; i++) {
    const packet = channel.receiveSync();
    const request = decode<{ bytes: Uint8Array; sequence: bigint }>(packet.bytes);
    channel.sendSync(2, encode(request));
  }
  self.postMessage('done');
};
