import { Channel } from '../src/channel';
import { decode, encode } from '../src/envelope';
import { PacketKind } from '../src/protocol';
self.onmessage = event => {
  const channel = new Channel(event.data.buffer);
  for (let i = 0; i < event.data.count; i++) {
    const packet = channel.receiveSync();
    const request = decode<{ bytes: Uint8Array; sequence: bigint }>(packet.bytes);
    channel.sendSync(PacketKind.NativeResult, encode(request));
  }
  self.postMessage('done');
};
