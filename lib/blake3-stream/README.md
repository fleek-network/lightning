# Blake3 Stream

A simple blake3 stream implementation that encodes and decodes files using segments of proofs from [blake3-tree](../blake3_tree).

An encoded stream looks like:

```text
[ length prefix (u64) ] [ proof segment ] [ block ] [ block ] [ proof segment ] [ block ] ...
```

## Benchmarks 

### Encode

|           | `KiB`       | `MiB`       |
|:----------|:------------|:------------|
| **`1`**   | `143.94 ns` | `524.90 us` |
| **`2`**   | `167.66 ns` | `1.29 ms`   |
| **`4`**   | `190.37 ns` | `2.23 ms`   |
| **`8`**   | `265.63 ns` | `3.92 ms`   |
| **`16`**  | `439.38 ns` | `2.39 ms`   |
| **`32`**  | `1.39 us`   | `12.11 ms`  |
| **`64`**  | `2.61 us`   | `23.06 ms`  |
| **`128`** | `5.34 us`   | `42.75 ms`  |
| **`256`** | `10.07 us`  | `87.60 ms`  |
| **`512`** | `28.70 us`  | `169.17 ms` |

### Verified Decode

|           | `KiB`       | `MiB`       |
|:----------|:------------|:------------|
| **`1`**   | `4.20 us`   | `319.50 us` |
| **`2`**   | `4.31 us`   | `637.65 us` |
| **`4`**   | `4.74 us`   | `1.30 ms`   |
| **`8`**   | `5.61 us`   | `2.68 ms`   |
| **`16`**  | `6.96 us`   | `5.58 ms`   |
| **`32`**  | `10.30 us`  | `13.68 ms`  |
| **`64`**  | `17.40 us`  | `27.30 ms`  |
| **`128`** | `30.34 us`  | `53.69 ms`  |
| **`256`** | `74.27 us`  | `107.16 ms` |
| **`512`** | `161.76 us` | `213.68 ms` |

---
Made with [criterion-table](https://github.com/nu11ptr/criterion-table)

