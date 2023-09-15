import { hash } from "./blake3.ts";
import { hash as wasmHash } from "../../blake3-wasm/pkg/blake3_wasm.js";
import { sha256 } from "https://denopkg.com/chiefbiiko/sha256@v1.0.0/mod.ts";

const SIZE: [string, number][] = [
  ["0B", 0],
  ["32B", 32],
  ["48B", 48],
  ["96B", 96],
  ["128B", 128],
  ["256B", 256],
  ["512B", 512],
  ["1Kib", 1 * 1024],
  ["8Kib", 8 * 1024],
  ["16Kib", 16 * 1024],
  ["32Kib", 32 * 1024],
  ["64Kib", 64 * 1024],
  ["128Kib", 128 * 1024],
  ["256Kib", 256 * 1024],
  ["512Kib", 512 * 1024],
  ["1MB", 1024 * 1024],
  ["100MB", 100 * 1024 * 1024],
];

for (const [group, size] of SIZE) {
  const input = new Uint8Array(size);

  Deno.bench({
    name: `Sha256 ${group}`,
    group,
    fn() {
      sha256(input);
    },
  });

  Deno.bench({
    name: `Optimized ${group}`,
    group,
    fn() {
      hash(input);
    },
  });

  Deno.bench({
    name: `WASM ${group}`,
    group,
    fn() {
      wasmHash(input);
    },
  });
}
