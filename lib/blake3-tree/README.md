# Blake3 Tree

This library provides an optimized and SIMD-enabled incremental Blake3 verifier.

## Why

The implementation of verifiable stream done in [bao](https://github.com/oconnor663/bao) does not utilize
the happy and fast route of the Blake3 implementation. There also other times that you want a good API and
representation.

In any case you can rely on this library to operate under those circumstances.

## Benchmarks

We care more about optimizing a server that is generating proofs. Since it is likely serving many clients
so while implementing these structures and functionalities we focused on making sure the server side is
as fast and efficient as possible.

The benchmark results should that generating a proof for a content takes less than 130 nanoseconds, the numbers on the
left are number of blocks (each block is 256KB) and there are two performance categories prefixed with `-beginning` and
`-resume`.

Since the proofs are sent over an stream we have two modes the first one generates/verifies the first proof in the
connection, and from that point forward we can assume that the client already has the proof for any data that comes
before the offset which we want to prove.


| Blk Count  | `gen-proof-beginning`          | `gen-proof-resume`              | `verify-proof-beginning`          | `verify-proof-resume`             |
|:-----------|:-------------------------------|:--------------------------------|:----------------------------------|:--------------------------------- |
| **`128`**  | `108.15 ns` (✅ **1.00x**)      | `40.00 ns` (🚀 **2.70x faster**) | `1.29 us` (❌ *11.89x slower*)     | `406.94 ns` (❌ *3.76x slower*)    |
| **`256`**  | `112.77 ns` (✅ **1.00x**)      | `39.54 ns` (🚀 **2.85x faster**) | `1.45 us` (❌ *12.87x slower*)     | `425.84 ns` (❌ *3.78x slower*)    |
| **`384`**  | `117.39 ns` (✅ **1.00x**)      | `40.30 ns` (🚀 **2.91x faster**) | `1.57 us` (❌ *13.39x slower*)     | `385.81 ns` (❌ *3.29x slower*)    |
| **`512`**  | `117.18 ns` (✅ **1.00x**)      | `40.08 ns` (🚀 **2.92x faster**) | `1.64 us` (❌ *13.99x slower*)     | `429.25 ns` (❌ *3.66x slower*)    |
| **`640`**  | `118.22 ns` (✅ **1.00x**)      | `41.03 ns` (🚀 **2.88x faster**) | `1.68 us` (❌ *14.25x slower*)     | `444.00 ns` (❌ *3.76x slower*)    |
| **`768`**  | `123.92 ns` (✅ **1.00x**)      | `40.70 ns` (🚀 **3.04x faster**) | `1.77 us` (❌ *14.31x slower*)     | `464.59 ns` (❌ *3.75x slower*)    |
| **`896`**  | `127.28 ns` (✅ **1.00x**)      | `42.33 ns` (🚀 **3.01x faster**) | `1.83 us` (❌ *14.37x slower*)     | `419.08 ns` (❌ *3.29x slower*)    |
| **`1024`** | `123.98 ns` (✅ **1.00x**)      | `39.82 ns` (🚀 **3.11x faster**) | `1.82 us` (❌ *14.66x slower*)     | `456.85 ns` (❌ *3.69x slower*)    |
| **`1152`** | `123.36 ns` (✅ **1.00x**)      | `40.60 ns` (🚀 **3.04x faster**) | `2.00 us` (❌ *16.23x slower*)     | `504.14 ns` (❌ *4.09x slower*)    |
| **`1280`** | `125.86 ns` (✅ **1.00x**)      | `40.44 ns` (🚀 **3.11x faster**) | `1.92 us` (❌ *15.25x slower*)     | `404.97 ns` (❌ *3.22x slower*)    |
| **`1408`** | `122.72 ns` (✅ **1.00x**)      | `42.42 ns` (🚀 **2.89x faster**) | `2.04 us` (❌ *16.62x slower*)     | `409.35 ns` (❌ *3.34x slower*)    |
| **`1536`** | `124.84 ns` (✅ **1.00x**)      | `40.72 ns` (🚀 **3.07x faster**) | `1.95 us` (❌ *15.61x slower*)     | `480.34 ns` (❌ *3.85x slower*)    |
| **`1664`** | `129.51 ns` (✅ **1.00x**)      | `41.09 ns` (🚀 **3.15x faster**) | `1.93 us` (❌ *14.88x slower*)     | `430.46 ns` (❌ *3.32x slower*)    |
| **`1792`** | `129.69 ns` (✅ **1.00x**)      | `41.68 ns` (🚀 **3.11x faster**) | `2.04 us` (❌ *15.70x slower*)     | `452.77 ns` (❌ *3.49x slower*)    |
| **`1920`** | `127.72 ns` (✅ **1.00x**)      | `40.97 ns` (🚀 **3.12x faster**) | `2.00 us` (❌ *15.63x slower*)     | `526.98 ns` (❌ *4.13x slower*)    |

---
Made with [criterion-table](https://github.com/nu11ptr/criterion-table)

