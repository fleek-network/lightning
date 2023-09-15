import { VECTOR } from "./testvec.ts";
import { hash } from "./blake3.ts";
import { assertStrictEquals } from "https://deno.land/std/assert/mod.ts";

function buf2hex(buffer: ArrayLike<number>) {
  return [...new Uint8Array(buffer)]
    .map((x) => x.toString(16).padStart(2, "0"))
    .join("");
}

Deno.test({
  name: "TEST VECTOR",
  fn() {
    const buffer = new Uint8Array(128 * 1024);
    for (let i = 0; i < buffer.length; ++i) {
      buffer[i] = i & 0xff;
    }

    for (let id = 0; id < VECTOR.length; ++id) {
      const [size, expectedHash] = VECTOR[id]! as [number, string];
      const input = new Uint8Array(buffer.buffer, 0, size);
      const actualHash = buf2hex(hash(input));
      // console.log(size, actualHash == expectedHash);
      assertStrictEquals(actualHash, expectedHash, `size=${size}` + input);
    }
  },
});
