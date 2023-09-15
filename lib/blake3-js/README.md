# Pure JavaScript Blake3

*And it's blazingly fast.*

This is the source code of a pure JavaScript implementation of the [Blake3](https://github.com/BLAKE3-team/BLAKE3)
algorithm, the implementations aims to be as efficient as possible. This includes both:

1. The size of the dependency.
2. The performance.

## Benchmarks

The following benchmarks compare the performance of this implementation vs web assembly compilation of the
original implementation.

```txt
cpu: Apple M1 Max
runtime: deno 1.36.4 (aarch64-apple-darwin)

file:///Users/parsa/Code/Fleek/lightning/lib/blake3-js/src/blake3_benches.ts
benchmark             time (avg)        iter/s             (min … max)       p75       p99      p995
---------------------------------------------------------------------- -----------------------------


Sha256 0B              2.13 µs/iter     469,357.1     (2.11 µs … 2.27 µs)   2.13 µs   2.27 µs   2.27 µs
Optimized 0B         337.63 ns/iter   2,961,804.2   (320.22 ns … 1.59 µs) 330.91 ns  374.4 ns   1.59 µs
WASM 0B               505.2 ns/iter   1,979,426.5 (497.21 ns … 530.78 ns) 507.89 ns 528.54 ns 530.78 ns

summary
  Optimized 0B
   1.5x faster than WASM 0B
   6.31x faster than Sha256 0B

Sha256 32B             2.14 µs/iter     468,242.9       (2.1 µs … 2.2 µs)   2.16 µs    2.2 µs    2.2 µs
Optimized 32B        366.45 ns/iter   2,728,899.1   (349.22 ns … 1.36 µs)  361.2 ns 380.66 ns   1.36 µs
WASM 32B             523.19 ns/iter   1,911,362.4 (517.56 ns … 539.71 ns)  527.6 ns  534.4 ns 539.71 ns

summary
  Optimized 32B
   1.43x faster than WASM 32B
   5.83x faster than Sha256 32B

Sha256 48B             2.19 µs/iter     456,054.1     (2.18 µs … 2.26 µs)   2.19 µs   2.26 µs   2.26 µs
Optimized 48B        368.49 ns/iter   2,713,751.5 (358.75 ns … 420.24 ns) 369.72 ns 392.18 ns 420.24 ns
WASM 48B             527.24 ns/iter   1,896,667.2 (519.55 ns … 567.62 ns) 531.56 ns 556.82 ns 567.62 ns

summary
  Optimized 48B
   1.43x faster than WASM 48B
   5.95x faster than Sha256 48B

Sha256 96B             3.18 µs/iter     314,309.8     (3.16 µs … 3.26 µs)   3.18 µs   3.26 µs   3.26 µs
Optimized 96B        442.74 ns/iter   2,258,652.9 (433.54 ns … 453.92 ns) 444.21 ns 453.71 ns 453.92 ns
WASM 96B             648.19 ns/iter   1,542,748.4 (628.38 ns … 700.15 ns) 653.95 ns 700.15 ns 700.15 ns

summary
  Optimized 96B
   1.46x faster than WASM 96B
   7.19x faster than Sha256 96B

Sha256 128B            4.11 µs/iter     243,058.6     (4.08 µs … 4.15 µs)   4.13 µs   4.15 µs   4.15 µs
Optimized 128B        455.5 ns/iter   2,195,410.3 (445.61 ns … 462.29 ns) 457.32 ns 461.67 ns 462.29 ns
WASM 128B               661 ns/iter   1,512,864.7 (631.73 ns … 706.71 ns) 664.86 ns 706.71 ns 706.71 ns

summary
  Optimized 128B
   1.45x faster than WASM 128B
   9.03x faster than Sha256 128B

Sha256 256B            6.13 µs/iter     163,137.9      (6.09 µs … 6.2 µs)   6.16 µs    6.2 µs    6.2 µs
Optimized 256B       629.88 ns/iter   1,587,600.7 (617.97 ns … 664.11 ns) 630.91 ns 664.11 ns 664.11 ns
WASM 256B            936.24 ns/iter   1,068,103.9 (923.91 ns … 968.28 ns) 939.22 ns 968.28 ns 968.28 ns

summary
  Optimized 256B
   1.49x faster than WASM 256B
   9.73x faster than Sha256 256B

Sha256 512B           10.23 µs/iter      97,713.5    (9.5 µs … 158.21 µs)  10.21 µs  13.21 µs  14.46 µs
Optimized 512B       976.67 ns/iter   1,023,882.5    (964.6 ns … 1.01 µs) 978.26 ns   1.01 µs   1.01 µs
WASM 512B              1.49 µs/iter     670,194.0     (1.48 µs … 1.58 µs)   1.49 µs   1.58 µs   1.58 µs

summary
  Optimized 512B
   1.53x faster than WASM 512B
   10.48x faster than Sha256 512B

Sha256 1Kib           18.33 µs/iter      54,555.4  (17.21 µs … 168.33 µs)   18.5 µs  22.54 µs  24.33 µs
Optimized 1Kib         1.66 µs/iter     603,355.2     (1.61 µs … 1.68 µs)   1.67 µs   1.68 µs   1.68 µs
WASM 1Kib              2.55 µs/iter     392,488.6     (2.53 µs … 2.57 µs)   2.55 µs   2.57 µs   2.57 µs

summary
  Optimized 1Kib
   1.54x faster than WASM 1Kib
   11.06x faster than Sha256 1Kib

Sha256 8Kib          131.62 µs/iter       7,597.8 (128.79 µs … 243.42 µs) 130.96 µs 144.58 µs  205.5 µs
Optimized 8Kib        12.07 µs/iter      82,863.8  (11.58 µs … 151.17 µs)  12.08 µs  12.71 µs  12.96 µs
WASM 8Kib             19.38 µs/iter      51,607.6  (18.58 µs … 876.04 µs)  19.33 µs     21 µs  23.08 µs

summary
  Optimized 8Kib
   1.61x faster than WASM 8Kib
   10.91x faster than Sha256 8Kib

Sha256 16Kib         264.66 µs/iter       3,778.4   (249.08 µs … 3.12 ms) 263.08 µs 299.12 µs 392.54 µs
Optimized 16Kib       23.92 µs/iter      41,800.8   (23.58 µs … 51.25 µs)  23.92 µs  26.58 µs  27.75 µs
WASM 16Kib            38.67 µs/iter      25,857.2     (38 µs … 198.12 µs)  38.58 µs  41.62 µs  43.71 µs

summary
  Optimized 16Kib
   1.62x faster than WASM 16Kib
   11.06x faster than Sha256 16Kib

Sha256 32Kib         524.95 µs/iter       1,904.9 (499.75 µs … 820.04 µs)  523.5 µs 663.21 µs 680.92 µs
Optimized 32Kib       47.72 µs/iter      20,953.8   (45.75 µs … 93.54 µs)   47.5 µs  52.58 µs  56.12 µs
WASM 32Kib            76.99 µs/iter      12,988.0  (74.12 µs … 319.17 µs)  76.75 µs  81.67 µs  84.92 µs

summary
  Optimized 32Kib
   1.61x faster than WASM 32Kib
   11x faster than Sha256 32Kib

Sha256 64Kib           1.04 ms/iter         960.7     (1.02 ms … 1.26 ms)   1.04 ms   1.19 ms   1.21 ms
Optimized 64Kib       93.18 µs/iter      10,731.5  (91.33 µs … 190.92 µs)  94.38 µs 102.83 µs 107.42 µs
WASM 64Kib           150.83 µs/iter       6,630.2 (149.25 µs … 189.46 µs) 150.29 µs 161.12 µs 168.71 µs

summary
  Optimized 64Kib
   1.62x faster than WASM 64Kib
   11.17x faster than Sha256 64Kib

Sha256 128Kib          2.03 ms/iter         493.0     (1.98 ms … 2.53 ms)   2.04 ms   2.24 ms   2.26 ms
Optimized 128Kib     190.29 µs/iter       5,255.1 (188.79 µs … 220.04 µs) 190.62 µs 198.96 µs 205.33 µs
WASM 128Kib          311.99 µs/iter       3,205.2    (299.83 µs … 394 µs)  312.5 µs 329.42 µs 338.29 µs

summary
  Optimized 128Kib
   1.64x faster than WASM 128Kib
   10.66x faster than Sha256 128Kib

Sha256 256Kib          4.15 ms/iter         240.8        (4 ms … 4.73 ms)   4.16 ms   4.46 ms   4.73 ms
Optimized 256Kib     380.61 µs/iter       2,627.3 (377.54 µs … 429.96 µs) 381.54 µs 401.21 µs 405.88 µs
WASM 256Kib          622.52 µs/iter       1,606.4    (617 µs … 669.25 µs) 623.58 µs 647.17 µs 656.79 µs

summary
  Optimized 256Kib
   1.64x faster than WASM 256Kib
   10.91x faster than Sha256 256Kib

Sha256 512Kib          8.25 ms/iter         121.3     (8.14 ms … 8.57 ms)   8.25 ms   8.57 ms   8.57 ms
Optimized 512Kib     764.08 µs/iter       1,308.8 (755.71 µs … 816.54 µs) 765.79 µs 797.17 µs  802.5 µs
WASM 512Kib            1.24 ms/iter         804.0     (1.23 ms … 1.33 ms)   1.25 ms   1.28 ms   1.29 ms

summary
  Optimized 512Kib
   1.63x faster than WASM 512Kib
   10.79x faster than Sha256 512Kib

Sha256 1MB            16.52 ms/iter          60.5   (16.31 ms … 16.87 ms)  16.67 ms  16.87 ms  16.87 ms
Optimized 1MB          1.53 ms/iter         655.7     (1.51 ms … 1.62 ms)   1.53 ms   1.57 ms    1.6 ms
WASM 1MB               2.48 ms/iter         402.9     (2.47 ms … 2.54 ms)   2.48 ms   2.54 ms   2.54 ms

summary
  Optimized 1MB
   1.63x faster than WASM 1MB
   10.83x faster than Sha256 1MB

Sha256 100MB            1.65 s/iter           0.6       (1.63 s … 1.67 s)    1.66 s    1.67 s    1.67 s
Optimized 100MB      151.68 ms/iter           6.6 (148.33 ms … 153.89 ms)  152.3 ms 153.89 ms 153.89 ms
WASM 100MB           247.05 ms/iter           4.0 (244.95 ms … 248.78 ms) 248.49 ms 248.78 ms 248.78 ms

summary
  Optimized 100MB
   1.63x faster than WASM 100MB
   10.87x faster than Sha256 100MB
```
