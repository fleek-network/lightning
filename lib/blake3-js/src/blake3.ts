// A u32 number.
type Word = number;

const OUT_LEN = 32;
const KEY_LEN = 32;
const BLOCK_LEN = 64;
const CHUNK_LEN = 1024;

const CHUNK_START = 1 << 0;
const CHUNK_END = 1 << 1;
const PARENT = 1 << 2;
const ROOT = 1 << 3;
const KEYED_HASH = 1 << 4;
const DERIVE_KEY_CONTEXT = 1 << 5;
const DERIVE_KEY_MATERIAL = 1 << 6;

// The first word of the decimal places of the square root of the first 8 primes.
const IV = new Uint32Array([
  0x6A09E667,
  0xBB67AE85,
  0x3C6EF372,
  0xA54FF53A,
  0x510E527F,
  0x9B05688C,
  0x1F83D9AB,
  0x5BE0CD19,
]);
// Blake3 is really little endian friendly, given +95% of devices running the client
// are indeed little endian, we can do some optimizations in regards to that.
const IsBigEndian = !(new Uint8Array(new Uint32Array([1]).buffer)[0]);
// Allocate two global arrays that we can reuse for every call to hash.
const blockWords = new Uint32Array(16);
// [[u32; 8]; 54]
const cvStackBuffer = new Uint32Array(432);

function compress(
  chainingValue: Uint32Array,
  chainingValueOffset: number,
  blockWords: Uint32Array,
  blockWordsOffset: number,
  counter: number,
  blockLen: Word,
  flags: Word,
  outBuffer: Uint32Array,
  outBufferOffset: number,
  keepLast8Byte: boolean,
) {
  let s_0 = chainingValue[chainingValueOffset + 0] | 0;
  let s_1 = chainingValue[chainingValueOffset + 1] | 0;
  let s_2 = chainingValue[chainingValueOffset + 2] | 0;
  let s_3 = chainingValue[chainingValueOffset + 3] | 0;
  let s_4 = chainingValue[chainingValueOffset + 4] | 0;
  let s_5 = chainingValue[chainingValueOffset + 5] | 0;
  let s_6 = chainingValue[chainingValueOffset + 6] | 0;
  let s_7 = chainingValue[chainingValueOffset + 7] | 0;
  let s_8 = 0x6A09E667;
  let s_9 = 0xBB67AE85;
  let s_10 = 0x3C6EF372;
  let s_11 = 0xA54FF53A;
  let s_12 = counter | 0;
  let s_13 = (counter >> 32) | 0;
  let s_14 = blockLen | 0;
  let s_15 = flags;
  const m_0 = blockWords[blockWordsOffset + 0];
  const m_1 = blockWords[blockWordsOffset + 1];
  const m_2 = blockWords[blockWordsOffset + 2];
  const m_3 = blockWords[blockWordsOffset + 3];
  const m_4 = blockWords[blockWordsOffset + 4];
  const m_5 = blockWords[blockWordsOffset + 5];
  const m_6 = blockWords[blockWordsOffset + 6];
  const m_7 = blockWords[blockWordsOffset + 7];
  const m_8 = blockWords[blockWordsOffset + 8];
  const m_9 = blockWords[blockWordsOffset + 9];
  const m_10 = blockWords[blockWordsOffset + 10];
  const m_11 = blockWords[blockWordsOffset + 11];
  const m_12 = blockWords[blockWordsOffset + 12];
  const m_13 = blockWords[blockWordsOffset + 13];
  const m_14 = blockWords[blockWordsOffset + 14];
  const m_15 = blockWords[blockWordsOffset + 15];
  s_0 = (((s_0 + s_4) | 0) + m_0) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_0 = (((s_0 + s_4) | 0) + m_1) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_1 = (((s_1 + s_5) | 0) + m_2) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_1 = (((s_1 + s_5) | 0) + m_3) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_2 = (((s_2 + s_6) | 0) + m_4) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_2 = (((s_2 + s_6) | 0) + m_5) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_3 = (((s_3 + s_7) | 0) + m_6) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_3 = (((s_3 + s_7) | 0) + m_7) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_0 = (((s_0 + s_5) | 0) + m_8) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_0 = (((s_0 + s_5) | 0) + m_9) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_1 = (((s_1 + s_6) | 0) + m_10) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_1 = (((s_1 + s_6) | 0) + m_11) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_2 = (((s_2 + s_7) | 0) + m_12) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_2 = (((s_2 + s_7) | 0) + m_13) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_3 = (((s_3 + s_4) | 0) + m_14) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_3 = (((s_3 + s_4) | 0) + m_15) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_0 = (((s_0 + s_4) | 0) + m_2) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_0 = (((s_0 + s_4) | 0) + m_6) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_1 = (((s_1 + s_5) | 0) + m_3) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_1 = (((s_1 + s_5) | 0) + m_10) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_2 = (((s_2 + s_6) | 0) + m_7) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_2 = (((s_2 + s_6) | 0) + m_0) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_3 = (((s_3 + s_7) | 0) + m_4) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_3 = (((s_3 + s_7) | 0) + m_13) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_0 = (((s_0 + s_5) | 0) + m_1) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_0 = (((s_0 + s_5) | 0) + m_11) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_1 = (((s_1 + s_6) | 0) + m_12) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_1 = (((s_1 + s_6) | 0) + m_5) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_2 = (((s_2 + s_7) | 0) + m_9) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_2 = (((s_2 + s_7) | 0) + m_14) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_3 = (((s_3 + s_4) | 0) + m_15) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_3 = (((s_3 + s_4) | 0) + m_8) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_0 = (((s_0 + s_4) | 0) + m_3) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_0 = (((s_0 + s_4) | 0) + m_4) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_1 = (((s_1 + s_5) | 0) + m_10) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_1 = (((s_1 + s_5) | 0) + m_12) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_2 = (((s_2 + s_6) | 0) + m_13) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_2 = (((s_2 + s_6) | 0) + m_2) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_3 = (((s_3 + s_7) | 0) + m_7) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_3 = (((s_3 + s_7) | 0) + m_14) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_0 = (((s_0 + s_5) | 0) + m_6) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_0 = (((s_0 + s_5) | 0) + m_5) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_1 = (((s_1 + s_6) | 0) + m_9) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_1 = (((s_1 + s_6) | 0) + m_0) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_2 = (((s_2 + s_7) | 0) + m_11) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_2 = (((s_2 + s_7) | 0) + m_15) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_3 = (((s_3 + s_4) | 0) + m_8) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_3 = (((s_3 + s_4) | 0) + m_1) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_0 = (((s_0 + s_4) | 0) + m_10) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_0 = (((s_0 + s_4) | 0) + m_7) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_1 = (((s_1 + s_5) | 0) + m_12) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_1 = (((s_1 + s_5) | 0) + m_9) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_2 = (((s_2 + s_6) | 0) + m_14) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_2 = (((s_2 + s_6) | 0) + m_3) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_3 = (((s_3 + s_7) | 0) + m_13) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_3 = (((s_3 + s_7) | 0) + m_15) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_0 = (((s_0 + s_5) | 0) + m_4) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_0 = (((s_0 + s_5) | 0) + m_0) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_1 = (((s_1 + s_6) | 0) + m_11) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_1 = (((s_1 + s_6) | 0) + m_2) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_2 = (((s_2 + s_7) | 0) + m_5) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_2 = (((s_2 + s_7) | 0) + m_8) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_3 = (((s_3 + s_4) | 0) + m_1) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_3 = (((s_3 + s_4) | 0) + m_6) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_0 = (((s_0 + s_4) | 0) + m_12) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_0 = (((s_0 + s_4) | 0) + m_13) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_1 = (((s_1 + s_5) | 0) + m_9) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_1 = (((s_1 + s_5) | 0) + m_11) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_2 = (((s_2 + s_6) | 0) + m_15) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_2 = (((s_2 + s_6) | 0) + m_10) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_3 = (((s_3 + s_7) | 0) + m_14) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_3 = (((s_3 + s_7) | 0) + m_8) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_0 = (((s_0 + s_5) | 0) + m_7) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_0 = (((s_0 + s_5) | 0) + m_2) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_1 = (((s_1 + s_6) | 0) + m_5) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_1 = (((s_1 + s_6) | 0) + m_3) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_2 = (((s_2 + s_7) | 0) + m_0) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_2 = (((s_2 + s_7) | 0) + m_1) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_3 = (((s_3 + s_4) | 0) + m_6) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_3 = (((s_3 + s_4) | 0) + m_4) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_0 = (((s_0 + s_4) | 0) + m_9) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_0 = (((s_0 + s_4) | 0) + m_14) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_1 = (((s_1 + s_5) | 0) + m_11) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_1 = (((s_1 + s_5) | 0) + m_5) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_2 = (((s_2 + s_6) | 0) + m_8) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_2 = (((s_2 + s_6) | 0) + m_12) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_3 = (((s_3 + s_7) | 0) + m_15) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_3 = (((s_3 + s_7) | 0) + m_1) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_0 = (((s_0 + s_5) | 0) + m_13) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_0 = (((s_0 + s_5) | 0) + m_3) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_1 = (((s_1 + s_6) | 0) + m_0) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_1 = (((s_1 + s_6) | 0) + m_10) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_2 = (((s_2 + s_7) | 0) + m_2) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_2 = (((s_2 + s_7) | 0) + m_6) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_3 = (((s_3 + s_4) | 0) + m_4) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_3 = (((s_3 + s_4) | 0) + m_7) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_0 = (((s_0 + s_4) | 0) + m_11) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_0 = (((s_0 + s_4) | 0) + m_15) | 0;
  s_12 ^= s_0;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_8 = (s_8 + s_12) | 0;
  s_4 ^= s_8;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  s_1 = (((s_1 + s_5) | 0) + m_5) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_1 = (((s_1 + s_5) | 0) + m_0) | 0;
  s_13 ^= s_1;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_9 = (s_9 + s_13) | 0;
  s_5 ^= s_9;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_2 = (((s_2 + s_6) | 0) + m_1) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_2 = (((s_2 + s_6) | 0) + m_9) | 0;
  s_14 ^= s_2;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_10 = (s_10 + s_14) | 0;
  s_6 ^= s_10;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_3 = (((s_3 + s_7) | 0) + m_8) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_3 = (((s_3 + s_7) | 0) + m_6) | 0;
  s_15 ^= s_3;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_11 = (s_11 + s_15) | 0;
  s_7 ^= s_11;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_0 = (((s_0 + s_5) | 0) + m_14) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 16) | (s_15 << 16);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 12) | (s_5 << 20);
  s_0 = (((s_0 + s_5) | 0) + m_10) | 0;
  s_15 ^= s_0;
  s_15 = (s_15 >>> 8) | (s_15 << 24);
  s_10 = (s_10 + s_15) | 0;
  s_5 ^= s_10;
  s_5 = (s_5 >>> 7) | (s_5 << 25);
  s_1 = (((s_1 + s_6) | 0) + m_2) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 16) | (s_12 << 16);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 12) | (s_6 << 20);
  s_1 = (((s_1 + s_6) | 0) + m_12) | 0;
  s_12 ^= s_1;
  s_12 = (s_12 >>> 8) | (s_12 << 24);
  s_11 = (s_11 + s_12) | 0;
  s_6 ^= s_11;
  s_6 = (s_6 >>> 7) | (s_6 << 25);
  s_2 = (((s_2 + s_7) | 0) + m_3) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 16) | (s_13 << 16);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 12) | (s_7 << 20);
  s_2 = (((s_2 + s_7) | 0) + m_4) | 0;
  s_13 ^= s_2;
  s_13 = (s_13 >>> 8) | (s_13 << 24);
  s_8 = (s_8 + s_13) | 0;
  s_7 ^= s_8;
  s_7 = (s_7 >>> 7) | (s_7 << 25);
  s_3 = (((s_3 + s_4) | 0) + m_7) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 16) | (s_14 << 16);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 12) | (s_4 << 20);
  s_3 = (((s_3 + s_4) | 0) + m_13) | 0;
  s_14 ^= s_3;
  s_14 = (s_14 >>> 8) | (s_14 << 24);
  s_9 = (s_9 + s_14) | 0;
  s_4 ^= s_9;
  s_4 = (s_4 >>> 7) | (s_4 << 25);
  if (keepLast8Byte) {
    outBuffer[outBufferOffset + 8] = s_8 ^
      chainingValue[chainingValueOffset + 0];
    outBuffer[outBufferOffset + 9] = s_9 ^
      chainingValue[chainingValueOffset + 1];
    outBuffer[outBufferOffset + 10] = s_10 ^
      chainingValue[chainingValueOffset + 2];
    outBuffer[outBufferOffset + 11] = s_11 ^
      chainingValue[chainingValueOffset + 3];
    outBuffer[outBufferOffset + 12] = s_12 ^
      chainingValue[chainingValueOffset + 4];
    outBuffer[outBufferOffset + 13] = s_13 ^
      chainingValue[chainingValueOffset + 5];
    outBuffer[outBufferOffset + 14] = s_14 ^
      chainingValue[chainingValueOffset + 6];
    outBuffer[outBufferOffset + 15] = s_15 ^
      chainingValue[chainingValueOffset + 7];
  }
  outBuffer[outBufferOffset + 0] = s_0 ^ s_8;
  outBuffer[outBufferOffset + 1] = s_1 ^ s_9;
  outBuffer[outBufferOffset + 2] = s_2 ^ s_10;
  outBuffer[outBufferOffset + 3] = s_3 ^ s_11;
  outBuffer[outBufferOffset + 4] = s_4 ^ s_12;
  outBuffer[outBufferOffset + 5] = s_5 ^ s_13;
  outBuffer[outBufferOffset + 6] = s_6 ^ s_14;
  outBuffer[outBufferOffset + 7] = s_7 ^ s_15;
}

// @assert_eq((array.length - offset) * 4 , words.length)
function readLittleEndianWordsFull(
  array: ArrayLike<number>,
  offset: number,
  words: Uint32Array,
) {
  for (let i = 0; i < words.length; ++i, offset += 4) {
    words[i] = array[offset] | (array[offset + 1] << 8) |
      (array[offset + 2] << 16) | (array[offset + 3] << 24);
  }
}

// @assert((array.length - offset) * 4 <= words.length)
function readLittleEndianWordsPartial(
  array: ArrayLike<number>,
  offset: number,
  words: Uint32Array,
) {
  let i = 0;
  // Read full multiples of four.
  for (; offset + 3 < array.length; ++i, offset += 4) {
    words[i] = array[offset] | (array[offset + 1] << 8) |
      (array[offset + 2] << 16) | (array[offset + 3] << 24);
  }
  // Fill the rest with zero.
  for (let j = i; j < words.length; ++j) {
    words[j] = 0;
  }
  // Read the last word. (If input not a multiple of 4).
  for (let s = 0; offset < array.length; s += 8, ++offset) {
    words[i] |= array[offset] << s;
  }
}

export function hash(payload: Uint8Array) {
  const payloadWords = new Uint32Array(
    payload.buffer,
    payload.byteOffset,
    payload.byteLength >> 2,
  );

  const flags = 0;
  const keyWords = IV;
  const length = payload.length;
  // Compute the number of bytes we can process knowing there is more data.
  const take = Math.max(0, ((length - 1) | 1023) - 1023);

  // The hasher state.
  let cvStackCurrentOffset = 0;
  let chunkCounter = 0;
  let offset = 0;

  for (; offset < take;) {
    cvStackBuffer.set(keyWords, cvStackCurrentOffset);

    for (let i = 0; i < 16; ++i, offset += 64) {
      if (IsBigEndian) {
        readLittleEndianWordsFull(payload, offset, blockWords);
      }

      compress(
        cvStackBuffer,
        cvStackCurrentOffset,
        IsBigEndian ? blockWords : payloadWords,
        IsBigEndian ? 0 : offset / 4,
        chunkCounter,
        BLOCK_LEN,
        flags | (i === 0 ? CHUNK_START : (i === 15 ? CHUNK_END : 0)),
        cvStackBuffer,
        cvStackCurrentOffset,
        false,
      );
    }

    // We added a new chunk.
    chunkCounter += 1;

    let totalChunks = chunkCounter;
    while ((totalChunks & 1) === 0) {
      // pop
      cvStackCurrentOffset -= 8;

      compress(
        keyWords,
        0,
        cvStackBuffer,
        cvStackCurrentOffset,
        0,
        BLOCK_LEN,
        flags | PARENT,
        cvStackBuffer,
        cvStackCurrentOffset,
        false,
      );
      totalChunks >>= 1;
    }

    // move the stack cursor forward, the last value in the stack is now pushed.
    cvStackCurrentOffset += 8;
  }

  // last chunk. it can be any number of blocks. in one special case where
  // n(remaining_bytes) <= BLOCK_LEN, the flag should be set to CHUNK_END
  // on the initial block.
  const remainingBytes = length - take;
  // remainingBytes > 0 -> no underflow.
  const fullBlocks = ((remainingBytes - 1) / 64) | 0;

  // Reset the current CV.
  cvStackBuffer.set(keyWords, cvStackCurrentOffset);

  for (let i = 0; i < fullBlocks; ++i, offset += 64) {
    if (IsBigEndian) {
      readLittleEndianWordsFull(payload, offset, blockWords);
    }

    compress(
      cvStackBuffer,
      cvStackCurrentOffset,
      IsBigEndian ? blockWords : payloadWords,
      IsBigEndian ? 0 : offset / 4,
      chunkCounter,
      BLOCK_LEN,
      flags | (i === 0 ? CHUNK_START : 0),
      cvStackBuffer,
      cvStackCurrentOffset,
      false,
    );
  }

  // There are two path in the code here. One case is that there is nothing in
  // the stack and that this block needs to be finalized. And the other is the
  // opposite, we have entries in the stack which we should merge.

  let finalChainingValue: Uint32Array;
  let finalBlockWords: Uint32Array;
  let finalBlockLen: number;
  let finalFlags: Word;

  readLittleEndianWordsPartial(payload, offset, blockWords);

  if (cvStackCurrentOffset === 0) {
    finalChainingValue = cvStackBuffer;
    finalBlockWords = blockWords;
    finalBlockLen = length - offset;
    finalFlags = flags | ROOT | CHUNK_END |
      (fullBlocks === 0 ? CHUNK_START : 0);
  } else {
    finalChainingValue = keyWords;
    finalBlockWords = cvStackBuffer;
    finalBlockLen = BLOCK_LEN;
    finalFlags = flags | PARENT | ROOT;

    // Compress the last chunk
    compress(
      cvStackBuffer,
      cvStackCurrentOffset,
      blockWords,
      0,
      chunkCounter,
      length - offset,
      flags | CHUNK_END | (fullBlocks === 0 ? CHUNK_START : 0),
      cvStackBuffer,
      cvStackCurrentOffset,
      false,
    );

    cvStackCurrentOffset += 8;

    while (cvStackCurrentOffset > 16) {
      cvStackCurrentOffset -= 8;

      compress(
        keyWords,
        0,
        cvStackBuffer,
        cvStackCurrentOffset,
        0,
        BLOCK_LEN,
        flags | PARENT,
        cvStackBuffer,
        cvStackCurrentOffset,
        false,
      );
    }
  }

  // 'loop: %outputBlockCounter
  const outputBlockCounter = 0;
  compress(
    finalChainingValue,
    0,
    finalBlockWords,
    0,
    outputBlockCounter,
    finalBlockLen,
    finalFlags,
    // reuse cvStackBuffer.
    cvStackBuffer,
    0,
    true,
  );

  return (new Uint8Array(cvStackBuffer.buffer, 0, 32)).slice(0);
}
