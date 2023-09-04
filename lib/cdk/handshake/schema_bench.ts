import { Challenge, Digest, Writer } from "./schema.ts";

Deno.bench({
  name: "u32: DataView [100k]",
  group: "write-u32",
  fn() {
    for (let i = 0; i < 100000; ++i) {
      const buffer = new ArrayBuffer(4);
      const view = new DataView(buffer);
      view.setUint32(0, 0x12345689);
    }
  },
});

Deno.bench({
  name: "u32: Uint8Array Loop [100k]",
  group: "write-u32",
  fn() {
    for (let i = 0; i < 100000; ++i) {
      const buffer = new ArrayBuffer(4);
      const u8 = new Uint8Array(buffer);
      let n = 0x12345689;
      for (let j = 0; j < 4; ++j) {
        u8[4 - j] = n & 0xff;
        n = n >> 2;
      }
    }
  },
});

Deno.bench({
  name: "u32: Writer.put_u32 [100k]",
  group: "write-u32",
  fn() {
    for (let i = 0; i < 100000; ++i) {
      const writer = new Writer(4);
      writer.putU32(0x12345689);
    }
  },
});

Deno.bench({
  name: "u32: Uint8Array Loop Unrolled [100k]",
  group: "write-u32",
  fn() {
    for (let i = 0; i < 100000; ++i) {
      const buffer = new ArrayBuffer(4);
      const u8 = new Uint8Array(buffer);
      let n = 0x12345689;
      u8[3] = n & 0xff;
      n = n >> 2;
      u8[2] = n & 0xff;
      n = n >> 2;
      u8[1] = n & 0xff;
      n = n >> 2;
      u8[0] = n & 0xff;
    }
  },
});

Deno.bench({
  name: "Challange Encode [1000x]",
  fn(b) {
    const challenge = {
      challenge: new Uint8Array(32) as Digest,
    };

    b.start();
    for (let i = 0; i < 1000; ++i) {
      Challenge.encode(challenge);
    }
    b.end();
  },
});

Deno.bench({
  name: "Challange Decode [1000x]",
  fn(b) {
    const bytes = Challenge.encode({
      challenge: new Uint8Array(32) as Digest,
    });

    b.start();
    for (let i = 0; i < 1000; ++i) {
      Challenge.decode(bytes);
    }
    b.end();
  },
});

// TODO: More...
